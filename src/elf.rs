use crate::package::BinaryPackage;
use flate2::read::GzDecoder;
use goblin::elf::{Elf, dynamic::*, header::*, program_header::*};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};
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
                extract_elfs_from_tar(GzDecoder::new(cursor))
            } else if name.ends_with(".xz") {
                extract_elfs_from_tar(XzDecoder::new(cursor))
            } else if name.ends_with(".zst") {
                extract_elfs_from_tar(Decoder::new(cursor)?)
            } else {
                // Uncompressed.
                extract_elfs_from_tar(cursor)
            };
        }
    }

    anyhow::bail!("data.tar not found in .deb")
}

/// Parses out and returns all ELF files within a TAR archive.
fn extract_elfs_from_tar<R: Read>(reader: R) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
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

pub fn detect_build_flags_from_elf(bytes: &[u8]) -> anyhow::Result<HashMap<String, bool>> {
    let elf = Elf::parse(bytes)?;

    let mut flags = HashMap::new();

    flags.insert(
        "-fstack-protector-strong".to_string(),
        elf.dynsyms
            .iter()
            .any(|sym| elf.dynstrtab.get_at(sym.st_name) == Some("__stack_chk_fail")),
    );

    let fortify_source = elf.dynsyms.iter().any(|sym| {
        elf.dynstrtab
            .get_at(sym.st_name)
            .map(|s| s.ends_with("_chk"))
            .unwrap_or(false)
    });
    flags.insert("-D_FORTIFY_SOURCE=2".to_string(), fortify_source);
    flags.insert("-D_FORTIFY_SOURCE=3".to_string(), fortify_source);

    flags.insert(
        "-Wl,-z,relro".to_string(),
        elf.program_headers
            .iter()
            .any(|ph| ph.p_type == PT_GNU_RELRO),
    );

    flags.insert(
        "-Wl,-z,now".to_string(),
        elf.dynamic
            .map(|d| d.dyns.iter().any(|x| x.d_tag == DT_BIND_NOW))
            .unwrap_or(false),
    );

    flags.insert("-fPIE".to_string(), elf.header.e_type == ET_DYN);

    flags.insert(
        "-fcf-protection".to_string(),
        elf.section_headers
            .iter()
            .any(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".note.gnu.property")),
    );

    flags.insert(
        "-g".to_string(),
        elf.section_headers.iter().any(|sh| {
            elf.shdr_strtab
                .get_at(sh.sh_name)
                .map(|n| n.starts_with(".debug_"))
                .unwrap_or(false)
        }),
    );

    Ok(flags)
}
