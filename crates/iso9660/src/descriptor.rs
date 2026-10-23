//! Volume descriptor set (ECMA-119 8).

use disc_image::{SECTOR_SIZE, SectorSource};

use crate::record::{le_u16, le_u32};
use crate::{Error, Result};

/// Sector of the first volume descriptor, relative to the volume.
pub(crate) const DESCRIPTOR_START: u64 = 16;

/// Upper bound on descriptors scanned before giving up on a terminator.
const MAX_DESCRIPTORS: u64 = 64;

const TYPE_PRIMARY: u8 = 1;
const TYPE_TERMINATOR: u8 = 255;

/// Fields of the primary volume descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryVolumeDescriptor {
    /// System identifier, e.g. `PLAYSTATION`.
    pub system_id: String,
    /// Volume identifier (the disc label).
    pub volume_id: String,
    /// Number of logical blocks in the volume.
    pub volume_space_size: u32,
    /// Logical block size; always 2048 for volumes this crate opens.
    pub logical_block_size: u16,
    /// Absolute LBA of the descriptor itself.
    pub lba: u64,
    pub(crate) root_record: [u8; 34],
}

/// Scans the descriptor set of the volume starting at `base_lba`.
pub(crate) fn find_primary(
    source: &impl SectorSource,
    base_lba: u64,
) -> Result<PrimaryVolumeDescriptor> {
    for index in 0..MAX_DESCRIPTORS {
        let lba = base_lba + DESCRIPTOR_START + index;
        let sector = source.read_sector(lba)?;
        if &sector[1..6] != b"CD001" || sector[6] != 1 {
            return Err(if index == 0 {
                Error::NotIso9660 { lba }
            } else {
                Error::Malformed {
                    lba,
                    reason: "descriptor set ends without a terminator",
                }
            });
        }
        match sector[0] {
            TYPE_PRIMARY => return parse_primary(lba, &sector),
            TYPE_TERMINATOR => return Err(Error::MissingPrimaryDescriptor),
            _ => {}
        }
    }
    Err(Error::MissingPrimaryDescriptor)
}

fn parse_primary(lba: u64, s: &[u8; SECTOR_SIZE]) -> Result<PrimaryVolumeDescriptor> {
    let logical_block_size = le_u16(&s[128..130]);
    if usize::from(logical_block_size) != SECTOR_SIZE {
        return Err(Error::Unsupported("logical block size other than 2048"));
    }
    let mut root_record = [0; 34];
    root_record.copy_from_slice(&s[156..190]);
    Ok(PrimaryVolumeDescriptor {
        system_id: text(&s[8..40]),
        volume_id: text(&s[40..72]),
        volume_space_size: le_u32(&s[80..84]),
        logical_block_size,
        lba,
        root_record,
    })
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches([' ', '\0'])
        .to_owned()
}
