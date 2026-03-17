use crate::{Package, Target, utility::get_with_retry};
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use std::io::{Cursor, Read};
use xz2::bufread::XzDecoder;
use zstd::Decoder;

pub async fn fetch_packages(client: &Client, target: &Target) -> anyhow::Result<Vec<Package>> {
    let suite = if target.pocket == "Release" {
        target.series.clone()
    } else {
        format!("{}-{}", target.series, target.pocket.to_lowercase())
    };
    let url = format!(
        "https://archive.ubuntu.com/ubuntu/dists/{}/{}/source/Sources.xz",
        suite, target.component
    );

    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    let mut content = String::new();
    let mut decoder = XzDecoder::new_multi_decoder(&bytes[..]);
    decoder.read_to_string(&mut content)?;

    std::fs::write("Sources", content.clone())?;

    let packages = content
        .split("\n\n")
        .filter(|block| !block.is_empty())
        .map(|block| {
            let mut package = Package::default();

            for line in block.lines() {
                if let Some((key, value)) = line.split_once(":") {
                    match key {
                        "Package" => package.name = value.trim().to_string(),
                        "Version" => package.version = value.trim().to_string(),
                        "Directory" => package.directory = value.trim().to_string(),
                        _ => (),
                    }
                }
            }

            package
        })
        .collect();

    Ok(packages)
}

#[derive(Debug, Deserialize)]
struct BuildRecord {
    source_package_version: String,
    build_log_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BuildRecords {
    next_collection_link: Option<String>,
    entries: Vec<BuildRecord>,
}

pub async fn fetch_build_log(
    client: &Client,
    target: &Target,
    package: &Package,
) -> anyhow::Result<Option<String>> {
    // Get build record.
    let mut url = format!(
        "https://api.launchpad.net/1.0/ubuntu/{}/{}?\
        ws.op=getBuildRecords&\
        build_state=Successfully%20built&\
        source_name={}&\
        pocket={}",
        target.series, target.arch, package.name, target.pocket
    );

    println!("{url}");

    // Iterate through pages of build records to find one that matches.
    loop {
        let response = get_with_retry(client, &url).await?;
        let text = response.text().await?;
        let page: BuildRecords = serde_json::de::from_str(&text[..]).unwrap();

        for record in page.entries {
            if let Some(build_log_url) = record.build_log_url
                && record.source_package_version == package.version
            {
                println!("{:?}", build_log_url);

                let response = get_with_retry(client, &build_log_url).await?;
                let mut build_log;

                // Decompress the build log, if necessary.
                if build_log_url.ends_with(".gz") {
                    let bytes = response.bytes().await?;
                    let mut decoder = GzDecoder::new(&bytes[..]);
                    build_log = String::new();
                    decoder.read_to_string(&mut build_log).unwrap();
                } else {
                    build_log = response.text().await?;
                }

                return Ok(Some(build_log));
            }
        }

        if let Some(next_page) = page.next_collection_link {
            url = next_page;
        } else {
            break;
        }
    }

    Ok(None)
}

/// Fetches the .deb file for a package in the Ubuntu archive and
/// extracts the ELF files within.
pub async fn fetch_elfs(
    client: &Client,
    target: &Target,
    package: &Package,
) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let url = format!(
        "https://archive.ubuntu.com/ubuntu/{}/{}_{}_{}.deb",
        package.directory, package.name, package.version, target.arch
    );

    println!("{url}");

    let response = get_with_retry(client, &url).await?;
    let bytes = response.bytes().await?;
    let mut archive = ar::Archive::new(&bytes[..]);

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
