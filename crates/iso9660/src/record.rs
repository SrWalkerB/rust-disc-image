//! Directory record layout (ECMA-119 9.1).

pub(crate) const FLAG_DIRECTORY: u8 = 0x02;
pub(crate) const FLAG_MULTI_EXTENT: u8 = 0x80;

const HEADER_LEN: usize = 33;

/// A directory record as stored on disc, with LBAs relative to its volume.
pub(crate) struct RawRecord<'a> {
    pub lba: u64,
    pub len: u32,
    pub flags: u8,
    pub interleaved: bool,
    pub name: &'a [u8],
}

impl RawRecord<'_> {
    pub fn is_dir(&self) -> bool {
        self.flags & FLAG_DIRECTORY != 0
    }

    /// The `.` and `..` entries, stored as the single bytes 0 and 1.
    pub fn is_self_or_parent(&self) -> bool {
        self.name == [0] || self.name == [1]
    }
}

/// Parses one record; `rec` is exactly the record's length. Returns `None`
/// when the record is too short for its header or name.
pub(crate) fn parse(rec: &[u8]) -> Option<RawRecord<'_>> {
    if rec.len() <= HEADER_LEN {
        return None;
    }
    let name_len = usize::from(rec[32]);
    if name_len == 0 || HEADER_LEN + name_len > rec.len() {
        return None;
    }
    let extended_attribute_len = u64::from(rec[1]);
    Some(RawRecord {
        lba: u64::from(le_u32(&rec[2..6])) + extended_attribute_len,
        len: le_u32(&rec[10..14]),
        flags: rec[25],
        interleaved: rec[26] != 0 || rec[27] != 0,
        name: &rec[HEADER_LEN..HEADER_LEN + name_len],
    })
}

/// Reads the little-endian half of a both-endian field.
pub(crate) fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

pub(crate) fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
