//! Disc identification tests over synthetic images.

use disc_image::MemoryImage;
use iso_test_builder::{Image, SECTOR, record};
use ps2_disc::{Error, Ps2Disc};

const CNF: &[u8] = b"BOOT2 = cdrom0:\\SLUS_123.45;1\r\nVER = 1.01\r\nVMODE = NTSC\r\n";
const ROOT: u32 = 20;

/// Layer 0 with `SYSTEM.CNF`, the boot ELF and an IRX module, spanning
/// `layer0_size` sectors.
fn layer0(img: &mut Image, layer0_size: u32) {
    img.primary(16, "GAME", layer0_size, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(
            ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[
                record(b"PAD.IRX;1", 23, 3, 0),
                record(b"SLUS_123.45;1", 22, 4, 0),
                record(b"SYSTEM.CNF;1", 21, CNF.len() as u32, 0),
            ],
        )
        .write(21, CNF)
        .write(22, b"\x7fELF")
        .write(23, b"irx");
}

fn open(img: Image) -> Ps2Disc<MemoryImage<Vec<u8>>> {
    Ps2Disc::open(MemoryImage::new(img.into_bytes()).unwrap()).unwrap()
}

#[test]
fn identifies_a_single_layer_disc() {
    let mut img = Image::new(40);
    layer0(&mut img, 32);
    let disc = open(img);
    assert_eq!(disc.layers().len(), 1);

    let cnf = disc.system_cnf().unwrap();
    assert_eq!(cnf.boot2(), Some("cdrom0:\\SLUS_123.45;1"));
    assert_eq!(cnf.version(), Some("1.01"));

    let elf = disc.boot_elf().unwrap();
    assert_eq!(elf.lba(), 22);
    assert_eq!(disc.layers()[0].read_to_vec(&elf).unwrap(), b"\x7fELF");

    let irx: Vec<_> = disc
        .irx_modules()
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(irx, ["/PAD.IRX"]);
}

#[test]
fn finds_the_second_layer_after_the_first_volume() {
    let layer0_size = 32u32;
    let base = layer0_size - 16;
    let mut img = Image::new(64);
    layer0(&mut img, layer0_size);
    img.primary(base + 16, "GAME", 64 - base, (ROOT, SECTOR as u32))
        .terminator(base + 17)
        .directory(
            base + ROOT,
            SECTOR as u32,
            (ROOT, SECTOR as u32),
            &[record(b"PART2.PAK;1", 30, 5, 0)],
        )
        .write(base + 30, b"layer");
    let disc = open(img);
    assert_eq!(disc.layers().len(), 2);
    assert_eq!(disc.layers()[1].base_lba(), u64::from(base));

    let files = disc.files().unwrap();
    let pak = files.iter().find(|f| f.path == "/PART2.PAK").unwrap();
    assert_eq!(pak.layer, 1);
    assert_eq!(pak.entry.lba(), u64::from(base + 30));
    assert_eq!(disc.layers()[1].read_to_vec(&pak.entry).unwrap(), b"layer");
}

#[test]
fn ignores_padding_after_a_single_layer() {
    let mut img = Image::new(48);
    layer0(&mut img, 32);
    assert_eq!(open(img).layers().len(), 1);
}

#[test]
fn reports_a_missing_system_cnf() {
    let mut img = Image::new(32);
    img.primary(16, "EMPTY", 32, (ROOT, SECTOR as u32))
        .terminator(17)
        .directory(ROOT, SECTOR as u32, (ROOT, SECTOR as u32), &[]);
    let disc = open(img);
    assert!(matches!(disc.system_cnf(), Err(Error::MissingSystemCnf)));
    assert!(matches!(disc.boot_elf(), Err(Error::MissingSystemCnf)));
}
