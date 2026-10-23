use crate::{Error, Result, SECTOR_SIZE, SectorSource, check_request};

/// An image held in memory, for tests and small images.
#[derive(Debug, Clone)]
pub struct MemoryImage<T> {
    data: T,
}

impl<T: AsRef<[u8]>> MemoryImage<T> {
    /// Wraps `data`, which must be a whole number of sectors long.
    pub fn new(data: T) -> Result<Self> {
        let len = data.as_ref().len();
        if !len.is_multiple_of(SECTOR_SIZE) {
            return Err(Error::UnalignedImage { len: len as u64 });
        }
        Ok(Self { data })
    }

    /// Returns the wrapped data.
    pub fn into_inner(self) -> T {
        self.data
    }
}

impl<T: AsRef<[u8]>> SectorSource for MemoryImage<T> {
    fn sector_count(&self) -> u64 {
        (self.data.as_ref().len() / SECTOR_SIZE) as u64
    }

    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        check_request(self.sector_count(), lba, buf.len())?;
        let start = lba as usize * SECTOR_SIZE;
        buf.copy_from_slice(&self.data.as_ref()[start..start + buf.len()]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sectors_from_memory() {
        let mut data = vec![0u8; SECTOR_SIZE * 2];
        data[SECTOR_SIZE] = 7;
        let image = MemoryImage::new(data).unwrap();
        assert_eq!(image.sector_count(), 2);
        assert_eq!(image.read_sector(1).unwrap()[0], 7);
    }

    #[test]
    fn rejects_partial_sectors() {
        assert!(matches!(
            MemoryImage::new(vec![0u8; 10]),
            Err(Error::UnalignedImage { len: 10 })
        ));
    }
}
