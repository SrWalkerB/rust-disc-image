//! Sector-level access to optical disc images.
//!
//! A [`SectorSource`] hands out 2048-byte sectors by logical block address
//! (LBA). File systems and console-specific readers are built on top of it and
//! never deal with the image format directly.

mod error;
mod file;
mod memory;

use std::sync::Arc;

pub use error::{Error, Result};
pub use file::IsoFile;
pub use memory::MemoryImage;

/// Size in bytes of a user-data sector (CD Mode 1, Mode 2 Form 1 and DVD).
pub const SECTOR_SIZE: usize = 2048;

/// A source of 2048-byte sectors addressed by LBA.
pub trait SectorSource {
    /// Number of sectors in the image.
    fn sector_count(&self) -> u64;

    /// Reads `buf.len() / SECTOR_SIZE` consecutive sectors starting at `lba`.
    ///
    /// `buf.len()` must be a multiple of [`SECTOR_SIZE`]. Implementations
    /// should validate requests with [`check_request`].
    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()>;

    /// Reads a single sector.
    fn read_sector(&self, lba: u64) -> Result<[u8; SECTOR_SIZE]> {
        let mut buf = [0; SECTOR_SIZE];
        self.read_sectors(lba, &mut buf)?;
        Ok(buf)
    }
}

/// Validates a read of `len` bytes at `lba` and returns the number of sectors
/// it covers.
pub fn check_request(sector_count: u64, lba: u64, len: usize) -> Result<u64> {
    if !len.is_multiple_of(SECTOR_SIZE) {
        return Err(Error::UnalignedBuffer { len });
    }
    let count = (len / SECTOR_SIZE) as u64;
    match lba.checked_add(count) {
        Some(end) if end <= sector_count => Ok(count),
        _ => Err(Error::OutOfRange {
            lba,
            count,
            sector_count,
        }),
    }
}

impl<T: SectorSource + ?Sized> SectorSource for &T {
    fn sector_count(&self) -> u64 {
        (**self).sector_count()
    }

    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_sectors(lba, buf)
    }
}

impl<T: SectorSource + ?Sized> SectorSource for Box<T> {
    fn sector_count(&self) -> u64 {
        (**self).sector_count()
    }

    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_sectors(lba, buf)
    }
}

impl<T: SectorSource + ?Sized> SectorSource for Arc<T> {
    fn sector_count(&self) -> u64 {
        (**self).sector_count()
    }

    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_sectors(lba, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_request_accepts_reads_up_to_the_last_sector() {
        assert_eq!(check_request(10, 0, SECTOR_SIZE * 10).unwrap(), 10);
        assert_eq!(check_request(10, 9, SECTOR_SIZE).unwrap(), 1);
        assert_eq!(check_request(10, 10, 0).unwrap(), 0);
    }

    #[test]
    fn check_request_rejects_reads_past_the_end() {
        assert!(matches!(
            check_request(10, 10, SECTOR_SIZE),
            Err(Error::OutOfRange {
                lba: 10,
                count: 1,
                sector_count: 10
            })
        ));
        assert!(matches!(
            check_request(10, u64::MAX, SECTOR_SIZE),
            Err(Error::OutOfRange { .. })
        ));
    }

    #[test]
    fn check_request_rejects_partial_sectors() {
        assert!(matches!(
            check_request(10, 0, 100),
            Err(Error::UnalignedBuffer { len: 100 })
        ));
    }
}
