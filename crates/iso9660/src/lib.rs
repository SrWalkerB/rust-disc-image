//! Read-only ISO 9660 file system over a [`SectorSource`](disc_image::SectorSource).
//!
//! A [`Volume`] is opened at a base LBA, so images holding more than one
//! volume (such as dual-layer DVD dumps) can open each one separately. Every
//! [`DirEntry`] reports absolute LBAs, ready for sector-level reads.

mod descriptor;
mod entry;
mod error;
mod record;
mod volume;

pub use descriptor::PrimaryVolumeDescriptor;
pub use entry::{DirEntry, Extent};
pub use error::{Error, Result};
pub use volume::Volume;
