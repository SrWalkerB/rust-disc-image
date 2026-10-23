use std::fs::File;
use std::io;
use std::path::Path;

use crate::{Error, Result, SECTOR_SIZE, SectorSource, check_request};

/// A plain `.iso` image: a file of consecutive 2048-byte sectors.
///
/// Reads are positional and never load the whole image, so a shared reference
/// can serve several readers at once.
#[derive(Debug)]
pub struct IsoFile {
    file: File,
    sector_count: u64,
}

impl IsoFile {
    /// Opens the image at `path` for reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_file(File::open(path)?)
    }

    /// Wraps an already opened file.
    pub fn from_file(file: File) -> Result<Self> {
        let len = file.metadata()?.len();
        if !len.is_multiple_of(SECTOR_SIZE as u64) {
            return Err(Error::UnalignedImage { len });
        }
        Ok(Self {
            file,
            sector_count: len / SECTOR_SIZE as u64,
        })
    }
}

impl SectorSource for IsoFile {
    fn sector_count(&self) -> u64 {
        self.sector_count
    }

    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        check_request(self.sector_count, lba, buf.len())?;
        read_exact_at(&self.file, buf, lba * SECTOR_SIZE as u64)?;
        Ok(())
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut std::mem::take(&mut buf)[n..];
                offset += n as u64;
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
compile_error!("IsoFile supports only Unix and Windows targets");

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str, contents: &[u8]) -> Self {
            let path =
                std::env::temp_dir().join(format!("disc-image-{}-{name}", std::process::id()));
            fs::write(&path, contents).unwrap();
            Self(path)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn reads_sectors_from_a_file() {
        let mut data = vec![0u8; SECTOR_SIZE * 3];
        data[SECTOR_SIZE] = 0xAB;
        data[SECTOR_SIZE * 3 - 1] = 0xCD;
        let tmp = TempFile::new("reads.iso", &data);

        let iso = IsoFile::open(&tmp.0).unwrap();
        assert_eq!(iso.sector_count(), 3);
        let mut buf = vec![0u8; SECTOR_SIZE * 2];
        iso.read_sectors(1, &mut buf).unwrap();
        assert_eq!(buf[0], 0xAB);
        assert_eq!(buf[SECTOR_SIZE * 2 - 1], 0xCD);
    }

    #[test]
    fn rejects_files_with_a_partial_sector() {
        let tmp = TempFile::new("partial.iso", &[0u8; SECTOR_SIZE + 1]);
        assert!(matches!(
            IsoFile::open(&tmp.0),
            Err(Error::UnalignedImage { len }) if len == SECTOR_SIZE as u64 + 1
        ));
    }

    #[test]
    fn rejects_reads_past_the_end() {
        let tmp = TempFile::new("short.iso", &[0u8; SECTOR_SIZE]);
        let iso = IsoFile::open(&tmp.0).unwrap();
        assert!(matches!(iso.read_sector(1), Err(Error::OutOfRange { .. })));
    }
}
