use crate::package::BinaryPackage;
use flate2::read::GzDecoder;
use goblin::elf::Elf;
use std::io::{Cursor, Read};
use xz2::bufread::XzDecoder;
use zstd::Decoder;

/// Extracts elfs from within a binary package by reading its .deb file.
pub async fn extract_elfs_from_binary_package(
    binary_package: &BinaryPackage,
) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let mut archive = ar::Archive::new(&binary_package.deb[..]);

    while let Some(entry) = archive.next_entry() {
        let mut entry = entry?;
        let name = String::from_utf8_lossy(entry.header().identifier()).to_string();

        if name.starts_with("data.tar") {
            let mut buffer = Vec::new();
            entry.read_to_end(&mut buffer)?;

            // Decompress and parse tar.
            let cursor = Cursor::new(buffer);
            return if name.ends_with(".gz") {
                gather_elfs_from_tar(GzDecoder::new(cursor))
            } else if name.ends_with(".xz") {
                gather_elfs_from_tar(XzDecoder::new(cursor))
            } else if name.ends_with(".zst") {
                gather_elfs_from_tar(Decoder::new(cursor)?)
            } else {
                // Uncompressed.
                gather_elfs_from_tar(cursor)
            };
        }
    }

    anyhow::bail!("data.tar not found in .deb")
}

/// Parses out and returns all ELF files within a TAR archive.
fn gather_elfs_from_tar<R: Read>(reader: R) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let mut archive = tar::Archive::new(reader);
    let mut results = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;

        // Skip non-files.
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path()?.to_string_lossy().into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;

        // If the file has the ELF starting bytes, gather it.
        if bytes.starts_with(b"\x7fELF") {
            results.push((path, bytes));
        }
    }

    Ok(results)
}

pub fn parse_elf(bytes: &[u8]) -> anyhow::Result<()> {
    let elf = Elf::parse(bytes)?;

    println!("type: {:?}", elf.header.e_type);
    println!("arch: {:?}", elf.header.e_machine);
    println!("entry: 0x{:x}", elf.entry);

    for ph in &elf.program_headers {
        println!("ph: type={:?} flags={:?}", ph.p_type, ph.p_flags);
    }

    for sym in &elf.syms {
        if let Some(name) = elf.strtab.get_at(sym.st_name) {
            println!("symbol: {}", name);
        }
    }

    Ok(())
}
