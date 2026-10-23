use crate::record::RawRecord;

/// A contiguous run of sectors holding part of a file or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    /// Absolute LBA of the first sector.
    pub lba: u64,
    /// Length in bytes.
    pub len: u32,
}

/// A file or directory in a volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    name: String,
    is_dir: bool,
    extents: Vec<Extent>,
    size: u64,
    pub(crate) interleaved: bool,
}

impl DirEntry {
    pub(crate) fn from_record(rec: &RawRecord<'_>, base_lba: u64) -> Self {
        let is_dir = rec.is_dir();
        Self {
            name: decode_name(rec.name, is_dir),
            is_dir,
            extents: vec![Extent {
                lba: base_lba + rec.lba,
                len: rec.len,
            }],
            size: u64::from(rec.len),
            interleaved: rec.interleaved,
        }
    }

    pub(crate) fn push_extent(&mut self, extent: Extent) {
        self.size += u64::from(extent.len);
        self.extents.push(extent);
    }

    /// Name without the `;1` version suffix. Empty for the root directory.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this entry is a directory.
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Size in bytes, summed over all extents.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Absolute LBA of the first extent.
    pub fn lba(&self) -> u64 {
        self.extents[0].lba
    }

    /// All extents in file order. Files over 4 GiB have more than one.
    pub fn extents(&self) -> &[Extent] {
        &self.extents
    }
}

/// Strips the `;version` suffix and the trailing dot of extensionless files.
pub(crate) fn decode_name(raw: &[u8], is_dir: bool) -> String {
    let name = String::from_utf8_lossy(raw);
    if is_dir {
        return name.into_owned();
    }
    let name = name.split_once(';').map_or(&*name, |(base, _)| base);
    name.strip_suffix('.').unwrap_or(name).to_owned()
}

#[cfg(test)]
mod tests {
    use super::decode_name;

    #[test]
    fn decodes_file_names() {
        assert_eq!(decode_name(b"SYSTEM.CNF;1", false), "SYSTEM.CNF");
        assert_eq!(decode_name(b"README.;1", false), "README");
        assert_eq!(decode_name(b"PLAIN", false), "PLAIN");
        assert_eq!(decode_name(b"DIR.X", true), "DIR.X");
    }
}
