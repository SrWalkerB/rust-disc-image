use std::{fmt, io};

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors produced while reading sectors.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The underlying reader failed.
    Io(io::Error),
    /// The buffer length is not a multiple of [`SECTOR_SIZE`](crate::SECTOR_SIZE).
    UnalignedBuffer {
        /// Length of the rejected buffer.
        len: usize,
    },
    /// The request reaches past the end of the image.
    OutOfRange {
        /// First sector requested.
        lba: u64,
        /// Number of sectors requested.
        count: u64,
        /// Number of sectors in the image.
        sector_count: u64,
    },
    /// The image length is not a whole number of sectors.
    UnalignedImage {
        /// Image length in bytes.
        len: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::UnalignedBuffer { len } => {
                write!(f, "buffer of {len} bytes is not a whole number of sectors")
            }
            Self::OutOfRange {
                lba,
                count,
                sector_count,
            } => write!(
                f,
                "read of {count} sector(s) at LBA {lba} is past the end of a {sector_count}-sector image"
            ),
            Self::UnalignedImage { len } => {
                write!(f, "image of {len} bytes is not a whole number of sectors")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
