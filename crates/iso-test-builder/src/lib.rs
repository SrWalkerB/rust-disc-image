//! Builds small synthetic ISO 9660 images for this workspace's tests.
//!
//! Every structure is placed at an explicit LBA, so tests control the exact
//! layout and can corrupt individual bytes afterwards.

/// Sector size used by the images.
pub const SECTOR: usize = 2048;

/// Directory flag of a directory record.
pub const DIR: u8 = 0x02;

/// Multi-extent flag: more extents of the same file follow.
pub const MULTI_EXTENT: u8 = 0x80;

/// An image under construction.
#[derive(Debug, Clone)]
pub struct Image {
    data: Vec<u8>,
}

impl Image {
    /// Creates a zero-filled image of `sectors` sectors.
    pub fn new(sectors: usize) -> Self {
        Self {
            data: vec![0; sectors * SECTOR],
        }
    }

    /// Writes `bytes` starting at the beginning of sector `lba`.
    pub fn write(&mut self, lba: u32, bytes: &[u8]) -> &mut Self {
        let start = lba as usize * SECTOR;
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        self
    }

    /// Writes a primary volume descriptor at `lba`.
    pub fn primary(
        &mut self,
        lba: u32,
        volume_id: &str,
        volume_space_size: u32,
        root: (u32, u32),
    ) -> &mut Self {
        let mut s = vec![0u8; SECTOR];
        s[0] = 1;
        s[1..6].copy_from_slice(b"CD001");
        s[6] = 1;
        pad_text(&mut s[8..40], "PLAYSTATION");
        pad_text(&mut s[40..72], volume_id);
        both_u32(&mut s[80..88], volume_space_size);
        both_u16(&mut s[120..124], 1);
        both_u16(&mut s[124..128], 1);
        both_u16(&mut s[128..132], SECTOR as u16);
        s[156..190].copy_from_slice(&record(&[0], root.0, root.1, DIR));
        s[881] = 1;
        self.write(lba, &s)
    }

    /// Writes a volume descriptor set terminator at `lba`.
    pub fn terminator(&mut self, lba: u32) -> &mut Self {
        let mut s = vec![0u8; SECTOR];
        s[0] = 255;
        s[1..6].copy_from_slice(b"CD001");
        s[6] = 1;
        self.write(lba, &s)
    }

    /// Writes a directory of `len` bytes at `lba`: the `.` and `..` records
    /// followed by `records`, packed so that no record crosses a sector.
    pub fn directory(
        &mut self,
        lba: u32,
        len: u32,
        parent: (u32, u32),
        records: &[Vec<u8>],
    ) -> &mut Self {
        let mut data = vec![0u8; len as usize];
        let mut off = 0;
        let all = [
            record(&[0], lba, len, DIR),
            record(&[1], parent.0, parent.1, DIR),
        ];
        for rec in all.iter().chain(records) {
            if off % SECTOR + rec.len() > SECTOR {
                off = off.next_multiple_of(SECTOR);
            }
            assert!(
                off + rec.len() <= data.len(),
                "directory records do not fit"
            );
            data[off..off + rec.len()].copy_from_slice(rec);
            off += rec.len();
        }
        self.write(lba, &data)
    }

    /// Mutable access to the raw bytes, for corrupting structures.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the finished image.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Encodes a directory record.
pub fn record(name: &[u8], lba: u32, len: u32, flags: u8) -> Vec<u8> {
    let rec_len = (33 + name.len()).next_multiple_of(2);
    let mut r = vec![0u8; rec_len];
    r[0] = rec_len as u8;
    both_u32(&mut r[2..10], lba);
    both_u32(&mut r[10..18], len);
    r[25] = flags;
    both_u16(&mut r[28..32], 1);
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}

fn both_u32(dst: &mut [u8], value: u32) {
    dst[..4].copy_from_slice(&value.to_le_bytes());
    dst[4..8].copy_from_slice(&value.to_be_bytes());
}

fn both_u16(dst: &mut [u8], value: u16) {
    dst[..2].copy_from_slice(&value.to_le_bytes());
    dst[2..4].copy_from_slice(&value.to_be_bytes());
}

fn pad_text(dst: &mut [u8], text: &str) {
    dst.fill(b' ');
    dst[..text.len()].copy_from_slice(text.as_bytes());
}
