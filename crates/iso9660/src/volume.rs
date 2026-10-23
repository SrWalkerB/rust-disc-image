use std::collections::HashSet;

use disc_image::{SECTOR_SIZE, SectorSource};

use crate::descriptor::{self, PrimaryVolumeDescriptor};
use crate::entry::{DirEntry, Extent, decode_name};
use crate::record::{self, FLAG_MULTI_EXTENT};
use crate::{Error, Result};

/// Directories larger than this are rejected instead of read.
const MAX_DIRECTORY_SIZE: u32 = 16 * 1024 * 1024;

/// Deepest directory nesting [`Volume::walk`] follows.
const MAX_DEPTH: usize = 64;

const SECTOR: u64 = SECTOR_SIZE as u64;

/// An open ISO 9660 volume.
#[derive(Debug)]
pub struct Volume<S> {
    source: S,
    base_lba: u64,
    descriptor: PrimaryVolumeDescriptor,
    root: DirEntry,
}

impl<S: SectorSource> Volume<S> {
    /// Opens the volume that starts at the first sector of `source`.
    pub fn open(source: S) -> Result<Self> {
        Self::open_at(source, 0)
    }

    /// Opens the volume whose sector 0 is `base_lba` in `source`. LBAs
    /// recorded inside the volume are relative to `base_lba`.
    pub fn open_at(source: S, base_lba: u64) -> Result<Self> {
        let descriptor = descriptor::find_primary(&source, base_lba)?;
        let root = record::parse(&descriptor.root_record)
            .filter(|rec| rec.is_dir())
            .map(|rec| DirEntry::from_record(&rec, base_lba))
            .ok_or(Error::Malformed {
                lba: descriptor.lba,
                reason: "invalid root directory record",
            })?;
        Ok(Self {
            source,
            base_lba,
            descriptor,
            root,
        })
    }

    /// The primary volume descriptor.
    pub fn descriptor(&self) -> &PrimaryVolumeDescriptor {
        &self.descriptor
    }

    /// Absolute LBA of the volume's sector 0.
    pub fn base_lba(&self) -> u64 {
        self.base_lba
    }

    /// The underlying sector source.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// The root directory.
    pub fn root(&self) -> &DirEntry {
        &self.root
    }

    /// Lists a directory, without the `.` and `..` entries.
    pub fn read_dir(&self, dir: &DirEntry) -> Result<Vec<DirEntry>> {
        if !dir.is_dir() {
            return Err(Error::NotADirectory);
        }
        let mut entries = Vec::new();
        let mut pending: Option<DirEntry> = None;
        for extent in dir.extents() {
            if extent.len > MAX_DIRECTORY_SIZE {
                return Err(Error::Unsupported("directory larger than 16 MiB"));
            }
            let len = u64::from(extent.len);
            for index in 0..len.div_ceil(SECTOR) {
                let lba = extent.lba + index;
                let sector = self.source.read_sector(lba)?;
                let limit = (len - index * SECTOR).min(SECTOR) as usize;
                let mut off = 0;
                while off < limit {
                    let rec_len = usize::from(sector[off]);
                    if rec_len == 0 {
                        // The rest of the sector is padding.
                        break;
                    }
                    let rec = sector
                        .get(off..off + rec_len)
                        .filter(|_| off + rec_len <= limit)
                        .and_then(record::parse)
                        .ok_or(Error::Malformed {
                            lba,
                            reason: "truncated directory record",
                        })?;
                    off += rec_len;
                    if rec.is_self_or_parent() {
                        continue;
                    }
                    let extent = Extent {
                        lba: self.base_lba + rec.lba,
                        len: rec.len,
                    };
                    let more = rec.flags & FLAG_MULTI_EXTENT != 0;
                    match pending.as_mut() {
                        Some(entry) => {
                            if decode_name(rec.name, rec.is_dir()) != entry.name() {
                                return Err(Error::Malformed {
                                    lba,
                                    reason: "multi-extent parts have different names",
                                });
                            }
                            entry.push_extent(extent);
                        }
                        None => pending = Some(DirEntry::from_record(&rec, self.base_lba)),
                    }
                    if !more {
                        entries.extend(pending.take());
                    }
                }
            }
        }
        if pending.is_some() {
            return Err(Error::Malformed {
                lba: dir.lba(),
                reason: "multi-extent file without a final extent",
            });
        }
        Ok(entries)
    }

    /// Finds an entry by path. Components are separated by `/` or `\`,
    /// compared without ASCII case and with any `;1` version suffix ignored.
    pub fn lookup(&self, path: &str) -> Result<Option<DirEntry>> {
        let mut current = self.root.clone();
        for component in path.split(['/', '\\']).filter(|c| !c.is_empty()) {
            if !current.is_dir() {
                return Ok(None);
            }
            let wanted = component
                .split_once(';')
                .map_or(component, |(base, _)| base);
            let wanted = wanted.strip_suffix('.').unwrap_or(wanted);
            match self
                .read_dir(&current)?
                .into_iter()
                .find(|entry| entry.name().eq_ignore_ascii_case(wanted))
            {
                Some(entry) => current = entry,
                None => return Ok(None),
            }
        }
        Ok(Some(current))
    }

    /// Lists every entry in the volume with its `/`-separated path, sorted
    /// by path.
    pub fn walk(&self) -> Result<Vec<(String, DirEntry)>> {
        let mut out = Vec::new();
        let mut visited = HashSet::from([self.root.lba()]);
        let mut stack = vec![(String::new(), self.root.clone(), 0)];
        while let Some((prefix, dir, depth)) = stack.pop() {
            for entry in self.read_dir(&dir)? {
                let path = format!("{prefix}/{}", entry.name());
                if entry.is_dir() {
                    if depth + 1 >= MAX_DEPTH {
                        return Err(Error::Unsupported("directory tree deeper than 64 levels"));
                    }
                    if !visited.insert(entry.lba()) {
                        return Err(Error::Malformed {
                            lba: entry.lba(),
                            reason: "directory cycle",
                        });
                    }
                    stack.push((path.clone(), entry.clone(), depth + 1));
                }
                out.push((path, entry));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    /// Reads file bytes starting at `offset` into `buf` and returns how many
    /// were read; fewer than `buf.len()` only at the end of the file.
    pub fn read_at(&self, file: &DirEntry, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if file.is_dir() {
            return Err(Error::NotAFile);
        }
        if file.interleaved {
            return Err(Error::Unsupported("interleaved files"));
        }
        let end = file.size().min(offset.saturating_add(buf.len() as u64));
        if offset >= end {
            return Ok(0);
        }
        let total = (end - offset) as usize;
        let mut done = 0;
        let mut extent_start = 0u64;
        for extent in file.extents() {
            let extent_end = extent_start + u64::from(extent.len);
            let mut pos = offset + done as u64;
            while done < total && pos >= extent_start && pos < extent_end {
                let rel = pos - extent_start;
                let lba = extent.lba + rel / SECTOR;
                let in_sector = (rel % SECTOR) as usize;
                let want = (total - done).min((extent_end - pos) as usize);
                if in_sector == 0 && want >= SECTOR_SIZE {
                    // Whole sectors go straight into the caller's buffer.
                    let n = want / SECTOR_SIZE * SECTOR_SIZE;
                    self.source.read_sectors(lba, &mut buf[done..done + n])?;
                    done += n;
                } else {
                    let sector = self.source.read_sector(lba)?;
                    let n = want.min(SECTOR_SIZE - in_sector);
                    buf[done..done + n].copy_from_slice(&sector[in_sector..in_sector + n]);
                    done += n;
                }
                pos = offset + done as u64;
            }
            if done == total {
                break;
            }
            extent_start = extent_end;
        }
        Ok(done)
    }

    /// Reads a whole file into memory. Check [`DirEntry::size`] first: game
    /// archives can be several gigabytes.
    pub fn read_to_vec(&self, file: &DirEntry) -> Result<Vec<u8>> {
        let len = usize::try_from(file.size())
            .map_err(|_| Error::Unsupported("file larger than the address space"))?;
        let mut buf = vec![0; len];
        let read = self.read_at(file, 0, &mut buf)?;
        buf.truncate(read);
        Ok(buf)
    }
}
