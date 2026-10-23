//! ELF32 little-endian header and program headers, as used by PS2 executables.

use crate::{Error, Result};

/// Size of the ELF32 file header.
pub(crate) const HEADER_LEN: usize = 52;
const PROGRAM_HEADER_LEN: usize = 32;
const MAX_PROGRAM_HEADERS: usize = 256;

/// `e_machine` value for MIPS, the EE's architecture.
pub const MACHINE_MIPS: u16 = 8;

/// `p_type` of a loadable segment.
pub const SEGMENT_LOAD: u32 = 1;

/// The parts of an ELF header needed to locate and load a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfHeader {
    /// `e_type`: 2 for executables.
    pub kind: u16,
    /// `e_machine`: [`MACHINE_MIPS`] on PS2 executables.
    pub machine: u16,
    /// Entry point address.
    pub entry: u32,
    /// Program headers in file order.
    pub segments: Vec<Segment>,
}

/// One program header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// `p_type`; [`SEGMENT_LOAD`] for loadable segments.
    pub kind: u32,
    /// Offset of the segment data in the file.
    pub offset: u32,
    /// Virtual load address.
    pub vaddr: u32,
    /// Bytes stored in the file.
    pub file_size: u32,
    /// Bytes occupied in memory; the rest past `file_size` is zeroed.
    pub mem_size: u32,
    /// `p_flags`: read 4, write 2, execute 1.
    pub flags: u32,
}

impl ElfHeader {
    /// Parses the file header at the start of `bytes`, returning it with the
    /// byte range that holds the program headers.
    pub(crate) fn parse_file_header(bytes: &[u8]) -> Result<(Self, std::ops::Range<usize>)> {
        let h = bytes
            .get(..HEADER_LEN)
            .ok_or(Error::Elf("file shorter than an ELF header"))?;
        if &h[..4] != b"\x7fELF" {
            return Err(Error::Elf("missing ELF magic"));
        }
        if h[4] != 1 || h[5] != 1 {
            return Err(Error::Elf("not a 32-bit little-endian ELF"));
        }
        let phoff = u32_at(h, 28) as usize;
        let phentsize = usize::from(u16_at(h, 42));
        let phnum = usize::from(u16_at(h, 44));
        if phnum > MAX_PROGRAM_HEADERS {
            return Err(Error::Elf("too many program headers"));
        }
        if phnum > 0 && phentsize != PROGRAM_HEADER_LEN {
            return Err(Error::Elf("unexpected program header size"));
        }
        let header = Self {
            kind: u16_at(h, 16),
            machine: u16_at(h, 18),
            entry: u32_at(h, 24),
            segments: Vec::new(),
        };
        Ok((header, phoff..phoff + phnum * PROGRAM_HEADER_LEN))
    }

    /// Parses a header and its program headers from the start of a file.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let (mut header, range) = Self::parse_file_header(bytes)?;
        let table = bytes
            .get(range)
            .ok_or(Error::Elf("program headers past the end of the file"))?;
        header.segments = parse_segments(table);
        Ok(header)
    }

    /// Loadable segments only.
    pub fn load_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter().filter(|s| s.kind == SEGMENT_LOAD)
    }
}

pub(crate) fn parse_segments(table: &[u8]) -> Vec<Segment> {
    table
        .chunks_exact(PROGRAM_HEADER_LEN)
        .map(|p| Segment {
            kind: u32_at(p, 0),
            offset: u32_at(p, 4),
            vaddr: u32_at(p, 8),
            file_size: u32_at(p, 16),
            mem_size: u32_at(p, 20),
            flags: u32_at(p, 24),
        })
        .collect()
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN + PROGRAM_HEADER_LEN];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 1;
        b[5] = 1;
        b[16..18].copy_from_slice(&2u16.to_le_bytes());
        b[18..20].copy_from_slice(&MACHINE_MIPS.to_le_bytes());
        b[24..28].copy_from_slice(&0x0010_0008u32.to_le_bytes());
        b[28..32].copy_from_slice(&(HEADER_LEN as u32).to_le_bytes());
        b[42..44].copy_from_slice(&(PROGRAM_HEADER_LEN as u16).to_le_bytes());
        b[44..46].copy_from_slice(&1u16.to_le_bytes());
        let p = &mut b[HEADER_LEN..];
        p[0..4].copy_from_slice(&SEGMENT_LOAD.to_le_bytes());
        p[4..8].copy_from_slice(&0x80u32.to_le_bytes());
        p[8..12].copy_from_slice(&0x0010_0000u32.to_le_bytes());
        p[16..20].copy_from_slice(&0x1000u32.to_le_bytes());
        p[20..24].copy_from_slice(&0x2000u32.to_le_bytes());
        p[24..28].copy_from_slice(&5u32.to_le_bytes());
        b
    }

    #[test]
    fn parses_header_and_segments() {
        let elf = ElfHeader::parse(&sample()).unwrap();
        assert_eq!(elf.kind, 2);
        assert_eq!(elf.machine, MACHINE_MIPS);
        assert_eq!(elf.entry, 0x0010_0008);
        assert_eq!(
            elf.load_segments().copied().collect::<Vec<_>>(),
            [Segment {
                kind: SEGMENT_LOAD,
                offset: 0x80,
                vaddr: 0x0010_0000,
                file_size: 0x1000,
                mem_size: 0x2000,
                flags: 5,
            }]
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(ElfHeader::parse(b"short").is_err());
        let mut bad = sample();
        bad[0] = 0;
        assert!(ElfHeader::parse(&bad).is_err());
        let mut truncated = sample();
        truncated.truncate(HEADER_LEN + 10);
        assert!(ElfHeader::parse(&truncated).is_err());
    }
}
