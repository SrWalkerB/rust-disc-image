//! Identifies PlayStation 2 discs: layers, `SYSTEM.CNF`, boot ELF and IOP
//! modules.
//!
//! Dual-layer DVD dumps hold two ISO 9660 volumes. The second starts 16
//! sectors before the end of the first volume, so that its descriptor set
//! begins right after layer 0, and its LBAs are relative to that start.
//! [`Ps2Disc::open`] finds both and reports absolute LBAs for every file.

mod error;
mod system_cnf;

use std::sync::Arc;

use disc_image::SectorSource;
use iso9660::{DirEntry, Volume};

pub use error::{Error, Result};
pub use system_cnf::{SystemCnf, serial_from_boot_path};

/// `SYSTEM.CNF` larger than this is rejected; real ones are under 100 bytes.
const MAX_SYSTEM_CNF_SIZE: u64 = 64 * 1024;

/// A file of the disc with the layer it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscFile {
    /// Layer index: 0, or 1 on dual-layer discs.
    pub layer: usize,
    /// `/`-separated path inside the layer.
    pub path: String,
    /// The entry, with absolute LBAs.
    pub entry: DirEntry,
}

/// An open PS2 disc.
#[derive(Debug)]
pub struct Ps2Disc<S> {
    layers: Vec<Volume<Arc<S>>>,
}

impl<S: SectorSource> Ps2Disc<S> {
    /// Opens layer 0 and, when the image continues past it, layer 1.
    pub fn open(source: S) -> Result<Self> {
        let source = Arc::new(source);
        let layer0 = Volume::open(Arc::clone(&source))?;
        let layer0_end = u64::from(layer0.descriptor().volume_space_size);
        let mut layers = vec![layer0];
        if layer0_end >= 16 && source.sector_count() > layer0_end {
            match Volume::open_at(Arc::clone(&source), layer0_end - 16) {
                Ok(layer1) => layers.push(layer1),
                // Padding after a single-layer volume, not a second layer.
                Err(iso9660::Error::NotIso9660 { .. }) => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(Self { layers })
    }

    /// The disc's volumes: one per layer.
    pub fn layers(&self) -> &[Volume<Arc<S>>] {
        &self.layers
    }

    /// Reads and parses `SYSTEM.CNF` from the root of layer 0.
    pub fn system_cnf(&self) -> Result<SystemCnf> {
        let layer0 = &self.layers[0];
        let entry = layer0
            .lookup("SYSTEM.CNF")?
            .filter(|e| !e.is_dir())
            .ok_or(Error::MissingSystemCnf)?;
        if entry.size() > MAX_SYSTEM_CNF_SIZE {
            return Err(Error::SystemCnfTooLarge(entry.size()));
        }
        Ok(SystemCnf::parse(&layer0.read_to_vec(&entry)?))
    }

    /// Finds the boot ELF named by `BOOT2` in `SYSTEM.CNF`.
    pub fn boot_elf(&self) -> Result<DirEntry> {
        let cnf = self.system_cnf()?;
        let boot2 = cnf.boot2().ok_or(Error::MissingBoot2)?;
        let path = system_cnf::boot_path(boot2)
            .ok_or_else(|| Error::UnsupportedBootDevice(boot2.to_owned()))?;
        self.layers[0]
            .lookup(path)?
            .filter(|e| !e.is_dir())
            .ok_or_else(|| Error::BootFileNotFound(path.to_owned()))
    }

    /// Lists every file on every layer, in layer then path order.
    pub fn files(&self) -> Result<Vec<DiscFile>> {
        let mut files = Vec::new();
        for (layer, volume) in self.layers.iter().enumerate() {
            for (path, entry) in volume.walk()? {
                if !entry.is_dir() {
                    files.push(DiscFile { layer, path, entry });
                }
            }
        }
        Ok(files)
    }

    /// Lists the IOP modules (`.IRX` files) on the disc.
    pub fn irx_modules(&self) -> Result<Vec<DiscFile>> {
        let mut files = self.files()?;
        files.retain(|f| {
            f.path
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("IRX"))
        });
        Ok(files)
    }
}
