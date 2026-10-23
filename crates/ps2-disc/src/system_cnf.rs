/// Parsed `SYSTEM.CNF`: `KEY = VALUE` lines in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemCnf {
    entries: Vec<(String, String)>,
}

impl SystemCnf {
    /// Parses the file contents. Lines without `=` are ignored.
    pub fn parse(bytes: &[u8]) -> Self {
        let text = String::from_utf8_lossy(bytes);
        let entries = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
            .filter(|(key, _)| !key.is_empty())
            .collect();
        Self { entries }
    }

    /// Value of `key`, compared without ASCII case. The first occurrence wins.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// `BOOT2`: the boot ELF, e.g. `cdrom0:\SCUS_974.81;1`.
    pub fn boot2(&self) -> Option<&str> {
        self.get("BOOT2")
    }

    /// `VER`: the disc version string.
    pub fn version(&self) -> Option<&str> {
        self.get("VER")
    }

    /// `VMODE`: `NTSC` or `PAL`.
    pub fn video_mode(&self) -> Option<&str> {
        self.get("VMODE")
    }

    /// All entries in file order.
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }
}

/// Strips the `cdrom0:` device from a `BOOT2` value, leaving the disc path.
pub(crate) fn boot_path(boot2: &str) -> Option<&str> {
    let (device, path) = boot2.split_once(':')?;
    device.eq_ignore_ascii_case("cdrom0").then_some(path)
}

/// Derives the product serial from a boot path: `\SCUS_974.81;1` becomes
/// `SCUS-97481`. Returns `None` when the file name does not follow the
/// `AAAA_DDD.DD` pattern.
pub fn serial_from_boot_path(path: &str) -> Option<String> {
    let name = path.rsplit(['\\', '/', ':']).next()?;
    let name = name.split_once(';').map_or(name, |(base, _)| base);
    let b = name.as_bytes();
    let valid = b.len() == 11
        && b[..4].iter().all(u8::is_ascii_alphabetic)
        && b[4] == b'_'
        && b[5..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'.'
        && b[9..].iter().all(u8::is_ascii_digit);
    valid.then(|| format!("{}-{}{}", &name[..4], &name[5..8], &name[9..]).to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_with_crlf_and_spacing() {
        let cnf = SystemCnf::parse(
            b"BOOT2 = cdrom0:\\SCUS_974.81;1\r\nVER=1.00\r\n\r\nVMODE = NTSC\r\njunk\r\n",
        );
        assert_eq!(cnf.boot2(), Some("cdrom0:\\SCUS_974.81;1"));
        assert_eq!(cnf.version(), Some("1.00"));
        assert_eq!(cnf.video_mode(), Some("NTSC"));
        assert_eq!(cnf.get("vmode"), Some("NTSC"));
        assert_eq!(cnf.entries().len(), 3);
    }

    #[test]
    fn strips_the_boot_device() {
        assert_eq!(boot_path("cdrom0:\\SLUS_123.45;1"), Some("\\SLUS_123.45;1"));
        assert_eq!(boot_path("CDROM0:\\A;1"), Some("\\A;1"));
        assert_eq!(boot_path("host:\\A;1"), None);
        assert_eq!(boot_path("no device"), None);
    }

    #[test]
    fn derives_serials() {
        assert_eq!(
            serial_from_boot_path("\\SCUS_974.81;1").as_deref(),
            Some("SCUS-97481")
        );
        assert_eq!(
            serial_from_boot_path("cdrom0:\\slus_201.23").as_deref(),
            Some("SLUS-20123")
        );
        assert_eq!(serial_from_boot_path("\\MAIN.ELF;1"), None);
    }
}
