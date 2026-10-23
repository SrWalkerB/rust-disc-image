//! Volume-level tests over synthetic images.

use disc_image::{MemoryImage, SECTOR_SIZE};
use iso_test_builder::{DIR, Image, MULTI_EXTENT, SECTOR, record};
use iso9660::{Error, Extent, Volume};

const ROOT: u32 = 20;
const SUB: u32 = 21;
const CNF: u32 = 22;
const BIG: u32 = 23;

/// Root with `SYSTEM.CNF`, `README.` (no extension), a two-sector `BIG.BIN`
/// and directory `DATA` holding `FILE.TXT`.
fn sample() -> Image {
    let mut img = Image::new(32);
    img.primary(16, "SAMPLE", 32, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(
            ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[
                record(b"BIG.BIN;1", BIG, 3000, 0),
                record(b"DATA", SUB, SECTOR as u32, DIR),
                record(b"README.;1", CNF, 0, 0),
                record(b"SYSTEM.CNF;1", CNF, 11, 0),
            ],
        )
        .directory(
            SUB,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[record(b"FILE.TXT;1", 26, 5, 0)],
        )
        .write(CNF, b"BOOT2 = X\r\n");
    let big: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    img.write(BIG, &big).write(26, b"hello");
    img
}

fn open(img: Image) -> Volume<MemoryImage<Vec<u8>>> {
    Volume::open(MemoryImage::new(img.into_bytes()).unwrap()).unwrap()
}

#[test]
fn reads_the_primary_volume_descriptor() {
    let volume = open(sample());
    let pvd = volume.descriptor();
    assert_eq!(pvd.system_id, "PLAYSTATION");
    assert_eq!(pvd.volume_id, "SAMPLE");
    assert_eq!(pvd.volume_space_size, 32);
    assert_eq!(pvd.logical_block_size, 2048);
    assert_eq!(pvd.lba, 16);
    assert_eq!(volume.root().lba(), u64::from(ROOT));
}

#[test]
fn lists_a_directory_without_self_and_parent() {
    let volume = open(sample());
    let names: Vec<_> = volume
        .read_dir(volume.root())
        .unwrap()
        .iter()
        .map(|e| (e.name().to_owned(), e.is_dir()))
        .collect();
    assert_eq!(
        names,
        [
            ("BIG.BIN".to_owned(), false),
            ("DATA".to_owned(), true),
            ("README".to_owned(), false),
            ("SYSTEM.CNF".to_owned(), false),
        ]
    );
}

#[test]
fn looks_up_paths_ignoring_case_version_and_separator() {
    let volume = open(sample());
    for path in ["DATA/FILE.TXT", "\\data\\file.txt;1", "/DATA//File.Txt"] {
        let entry = volume.lookup(path).unwrap().expect(path);
        assert_eq!(entry.lba(), 26);
        assert_eq!(entry.size(), 5);
    }
    assert!(volume.lookup("DATA/MISSING").unwrap().is_none());
    assert!(volume.lookup("SYSTEM.CNF/X").unwrap().is_none());
    assert_eq!(volume.lookup("").unwrap().unwrap().lba(), u64::from(ROOT));
}

#[test]
fn walks_the_tree_in_path_order() {
    let volume = open(sample());
    let paths: Vec<_> = volume.walk().unwrap().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        paths,
        [
            "/BIG.BIN",
            "/DATA",
            "/DATA/FILE.TXT",
            "/README",
            "/SYSTEM.CNF"
        ]
    );
}

#[test]
fn reads_whole_files_and_unaligned_ranges() {
    let volume = open(sample());
    let cnf = volume.lookup("SYSTEM.CNF").unwrap().unwrap();
    assert_eq!(volume.read_to_vec(&cnf).unwrap(), b"BOOT2 = X\r\n");

    let big = volume.lookup("BIG.BIN").unwrap().unwrap();
    let expected: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    assert_eq!(volume.read_to_vec(&big).unwrap(), expected);

    let mut buf = [0u8; 100];
    assert_eq!(volume.read_at(&big, 2000, &mut buf).unwrap(), 100);
    assert_eq!(&buf[..], &expected[2000..2100]);

    assert_eq!(volume.read_at(&big, 2950, &mut buf).unwrap(), 50);
    assert_eq!(&buf[..50], &expected[2950..]);
    assert_eq!(volume.read_at(&big, 3000, &mut buf).unwrap(), 0);

    let empty = volume.lookup("README").unwrap().unwrap();
    assert!(volume.read_to_vec(&empty).unwrap().is_empty());
}

#[test]
fn rejects_directory_and_file_misuse() {
    let volume = open(sample());
    let dir = volume.lookup("DATA").unwrap().unwrap();
    let file = volume.lookup("SYSTEM.CNF").unwrap().unwrap();
    assert!(matches!(volume.read_dir(&file), Err(Error::NotADirectory)));
    assert!(matches!(volume.read_to_vec(&dir), Err(Error::NotAFile)));
}

#[test]
fn packs_records_across_directory_sectors() {
    let names: Vec<String> = (0..120).map(|i| format!("FILE{i:03}.BIN;1")).collect();
    let records: Vec<_> = names
        .iter()
        .map(|n| record(n.as_bytes(), 30, 0, 0))
        .collect();
    let mut img = Image::new(32);
    img.primary(16, "MANY", 32, (ROOT, 3 * SECTOR as u32))
        .terminator(17)
        .directory(ROOT, 3 * SECTOR as u32, (ROOT, 3 * SECTOR as u32), &records);
    let volume = open(img);
    let entries = volume.read_dir(volume.root()).unwrap();
    assert_eq!(entries.len(), 120);
    assert_eq!(entries[119].name(), "FILE119.BIN");
}

#[test]
fn merges_multi_extent_files() {
    let mut img = Image::new(32);
    img.primary(16, "MULTI", 32, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(
            ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[
                record(b"SPLIT.BIN;1", 24, SECTOR as u32, MULTI_EXTENT),
                record(b"SPLIT.BIN;1", 28, 10, 0),
            ],
        )
        .write(24, &[1u8; SECTOR])
        .write(28, &[2u8; 10]);
    let volume = open(img);
    let entry = volume.lookup("SPLIT.BIN").unwrap().unwrap();
    assert_eq!(entry.size(), SECTOR as u64 + 10);
    assert_eq!(
        entry.extents(),
        [
            Extent {
                lba: 24,
                len: SECTOR as u32
            },
            Extent { lba: 28, len: 10 },
        ]
    );
    let data = volume.read_to_vec(&entry).unwrap();
    assert!(data[..SECTOR].iter().all(|&b| b == 1));
    assert!(data[SECTOR..].iter().all(|&b| b == 2));

    let mut buf = [0u8; 4];
    volume.read_at(&entry, SECTOR as u64 - 2, &mut buf).unwrap();
    assert_eq!(buf, [1, 1, 2, 2]);
}

#[test]
fn opens_a_volume_at_a_base_lba() {
    let base = 40u32;
    let mut img = Image::new(80);
    img.primary(base + 16, "SECOND", 40, (ROOT, SECTOR as u32))
        .terminator(base + 17)
        .directory(
            base + ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[record(b"PART2.PAK;1", 30, 4, 0)],
        )
        .write(base + 30, b"pak2");
    let volume = Volume::open_at(MemoryImage::new(img.into_bytes()).unwrap(), 40).unwrap();
    assert_eq!(volume.base_lba(), 40);
    assert_eq!(volume.root().lba(), u64::from(base + ROOT));
    let pak = volume.lookup("PART2.PAK").unwrap().unwrap();
    assert_eq!(pak.lba(), u64::from(base + 30));
    assert_eq!(volume.read_to_vec(&pak).unwrap(), b"pak2");
}

#[test]
fn rejects_images_without_a_volume_descriptor() {
    let image = MemoryImage::new(vec![0u8; 32 * SECTOR_SIZE]).unwrap();
    assert!(matches!(
        Volume::open(image),
        Err(Error::NotIso9660 { lba: 16 })
    ));
}

#[test]
fn rejects_a_descriptor_set_without_a_primary() {
    let mut img = Image::new(32);
    img.terminator(16);
    let image = MemoryImage::new(img.into_bytes()).unwrap();
    assert!(matches!(
        Volume::open(image),
        Err(Error::MissingPrimaryDescriptor)
    ));
}

#[test]
fn rejects_unsupported_block_sizes() {
    let mut img = sample();
    img.bytes_mut()[16 * SECTOR + 128..16 * SECTOR + 130].copy_from_slice(&512u16.to_le_bytes());
    let image = MemoryImage::new(img.into_bytes()).unwrap();
    assert!(matches!(Volume::open(image), Err(Error::Unsupported(_))));
}

#[test]
fn rejects_records_crossing_a_sector_boundary() {
    let names: Vec<String> = (0..60).map(|i| format!("FILE{i:03}.BIN;1")).collect();
    let records: Vec<_> = names
        .iter()
        .map(|n| record(n.as_bytes(), 30, 0, 0))
        .collect();
    let mut img = Image::new(32);
    img.primary(16, "CROSS", 32, (ROOT, 2 * SECTOR as u32))
        .terminator(17)
        .directory(ROOT, 2 * SECTOR as u32, (ROOT, 2 * SECTOR as u32), &records);
    // "." and ".." take 68 bytes and each 46-byte record follows, so the last
    // record of the first sector starts at 2000. Make it run past byte 2048.
    let last = ROOT as usize * SECTOR + 2000;
    assert_eq!(img.bytes_mut()[last], 46);
    img.bytes_mut()[last] = 60;
    let volume = open(img);
    let err = volume.read_dir(volume.root()).unwrap_err();
    assert!(
        matches!(err, Error::Malformed { lba, .. } if lba == u64::from(ROOT)),
        "{err}"
    );
}

#[test]
fn rejects_records_past_the_directory_length() {
    let mut img = sample();
    // Shrink the root to 200 bytes; its last record ends at byte 236.
    for at in [16 * SECTOR + 156 + 10, ROOT as usize * SECTOR + 10] {
        img.bytes_mut()[at..at + 4].copy_from_slice(&200u32.to_le_bytes());
    }
    let volume = open(img);
    assert!(matches!(
        volume.read_dir(volume.root()),
        Err(Error::Malformed {
            reason: "truncated directory record",
            ..
        })
    ));
}

#[test]
fn rejects_directory_cycles() {
    let mut img = Image::new(32);
    img.primary(16, "LOOP", 32, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(
            ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[record(b"SELF", ROOT, SECTOR as u32, DIR)],
        );
    let volume = open(img);
    assert!(matches!(
        volume.walk(),
        Err(Error::Malformed {
            reason: "directory cycle",
            ..
        })
    ));
}

#[test]
fn reports_reads_past_the_end_of_the_image() {
    let mut img = Image::new(24);
    img.primary(16, "SHORT", 24, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(
            ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[record(b"GONE.BIN;1", 500, 10, 0)],
        );
    let volume = open(img);
    let gone = volume.lookup("GONE.BIN").unwrap().unwrap();
    assert!(matches!(
        volume.read_to_vec(&gone),
        Err(Error::Disc(disc_image::Error::OutOfRange { .. }))
    ));
}
