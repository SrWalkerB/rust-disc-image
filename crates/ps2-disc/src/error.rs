use std::fmt;

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors produced while identifying a PS2 disc.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Reading the file system failed.
    Iso(iso9660::Error),
    /// Layer 0 has no `SYSTEM.CNF` in its root.
    MissingSystemCnf,
    /// `SYSTEM.CNF` is implausibly large.
    SystemCnfTooLarge(u64),
    /// `SYSTEM.CNF` has no `BOOT2` line; the disc may be a PS1 disc.
    MissingBoot2,
    /// `BOOT2` points somewhere other than `cdrom0:`.
    UnsupportedBootDevice(String),
    /// The file named by `BOOT2` is not on layer 0.
    BootFileNotFound(String),
    /// The boot ELF header is invalid.
    Elf(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Iso(err) => write!(f, "{err}"),
            Self::MissingSystemCnf => f.write_str("SYSTEM.CNF not found on layer 0"),
            Self::SystemCnfTooLarge(size) => write!(f, "SYSTEM.CNF is {size} bytes"),
            Self::MissingBoot2 => f.write_str("SYSTEM.CNF has no BOOT2 entry"),
            Self::UnsupportedBootDevice(boot2) => {
                write!(f, "BOOT2 does not point to cdrom0: {boot2}")
            }
            Self::BootFileNotFound(path) => write!(f, "boot file {path} not found on layer 0"),
            Self::Elf(reason) => write!(f, "invalid ELF: {reason}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Iso(err) => Some(err),
            _ => None,
        }
    }
}

impl From<iso9660::Error> for Error {
    fn from(err: iso9660::Error) -> Self {
        Self::Iso(err)
    }
}
