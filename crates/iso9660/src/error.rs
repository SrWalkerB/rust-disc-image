use std::fmt;

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors produced while reading an ISO 9660 volume.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Reading sectors failed.
    Disc(disc_image::Error),
    /// There is no ISO 9660 volume descriptor at the expected sector.
    NotIso9660 {
        /// Sector that was checked.
        lba: u64,
    },
    /// The descriptor set ends without a primary volume descriptor.
    MissingPrimaryDescriptor,
    /// The volume uses a feature this crate does not read.
    Unsupported(&'static str),
    /// On-disc structures are inconsistent.
    Malformed {
        /// Sector where the problem was found.
        lba: u64,
        /// What is wrong.
        reason: &'static str,
    },
    /// A directory operation was given a file.
    NotADirectory,
    /// A file operation was given a directory.
    NotAFile,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disc(err) => write!(f, "{err}"),
            Self::NotIso9660 { lba } => {
                write!(f, "no ISO 9660 volume descriptor at LBA {lba}")
            }
            Self::MissingPrimaryDescriptor => f.write_str("no primary volume descriptor"),
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
            Self::Malformed { lba, reason } => write!(f, "malformed volume at LBA {lba}: {reason}"),
            Self::NotADirectory => f.write_str("not a directory"),
            Self::NotAFile => f.write_str("not a file"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Disc(err) => Some(err),
            _ => None,
        }
    }
}

impl From<disc_image::Error> for Error {
    fn from(err: disc_image::Error) -> Self {
        Self::Disc(err)
    }
}
