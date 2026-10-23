//! Prints the layers, SYSTEM.CNF, boot ELF and file list of a PS2 disc image.
//!
//! Usage: `cargo run -p ps2-disc --example ps2-info -- <image.iso>`

use std::process::ExitCode;

use disc_image::{IsoFile, SectorSource};
use ps2_disc::{Ps2Disc, serial_from_boot_path};

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: ps2-info <image.iso>");
        return ExitCode::FAILURE;
    };
    match run(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &std::ffi::OsStr) -> Result<(), Box<dyn std::error::Error>> {
    let image = IsoFile::open(path)?;
    println!("image: {} sectors", image.sector_count());
    let disc = Ps2Disc::open(image)?;

    for (index, layer) in disc.layers().iter().enumerate() {
        let pvd = layer.descriptor();
        println!(
            "layer {index}: system {:?}, volume {:?}, base LBA {}, {} sectors",
            pvd.system_id,
            pvd.volume_id,
            layer.base_lba(),
            pvd.volume_space_size
        );
    }

    let cnf = disc.system_cnf()?;
    println!("SYSTEM.CNF:");
    for (key, value) in cnf.entries() {
        println!("  {key} = {value}");
    }
    if let Some(serial) = cnf.boot2().and_then(serial_from_boot_path) {
        println!("serial: {serial}");
    }
    let elf = disc.boot_elf()?;
    println!(
        "boot ELF: {} at LBA {}, {} bytes",
        elf.name(),
        elf.lba(),
        elf.size()
    );
    let header = disc.boot_elf_header()?;
    println!("  machine {}, entry {:#010x}", header.machine, header.entry);
    for seg in &header.segments {
        println!(
            "  segment type {} offset {:#x} vaddr {:#010x} file {:#x} mem {:#x} flags {}",
            seg.kind, seg.offset, seg.vaddr, seg.file_size, seg.mem_size, seg.flags
        );
    }

    println!("files:");
    for file in disc.files()? {
        println!(
            "  [{}] {:<20} LBA {:>8}  {:>12} bytes",
            file.layer,
            file.path,
            file.entry.lba(),
            file.entry.size()
        );
    }
    Ok(())
}
