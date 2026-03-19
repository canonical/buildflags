use crate::{Target, package};
use reqwest::Client;
use std::{collections::HashMap, io::Read};
use xz2::bufread::XzDecoder;

#[derive(Default, Debug, Clone)]
pub struct SourcePackage {
    pub name: String,
    pub version: String,
}

#[derive(Default, Debug, Clone)]
pub struct BinaryPackage {
    pub name: String,
    pub version: String,
    pub deb: Vec<u8>,
}

pub async fn fetch_packages(
    client: &Client,
    target: &Target,
) -> anyhow::Result<Vec<(SourcePackage, Vec<BinaryPackage>)>> {
    // Determine archive url.
    let archive_url = match &target.arch[..] {
        "amd64" | "i386" => "https://archive.ubuntu.com/ubuntu",
        _ => "https://ports.ubuntu.com/ubuntu-ports",
    };

    // Get Sources.xz.
    let suite = if target.pocket == "Release" {
        target.series.clone()
    } else {
        format!("{}-{}", target.series, target.pocket.to_lowercase())
    };
    let url = format!(
        "{}/dists/{}/{}/source/Sources.xz",
        archive_url, suite, target.component
    );

    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;

    // Decompress Sources.xz.
    let mut content = String::new();
    let mut decoder = XzDecoder::new_multi_decoder(&bytes[..]);
    decoder.read_to_string(&mut content)?;
    std::fs::write("Sources", content.clone())?;

    // Parse Sources into source packages.
    let mut packages = HashMap::new();
    for block in content.split("\n\n").filter(|block| !block.is_empty()) {
        let mut package = SourcePackage::default();

        for line in block.lines() {
            if let Some((key, value)) = line.split_once(":") {
                match key {
                    "Package" => package.name = value.trim().to_string(),
                    "Version" => package.version = value.trim().to_string(),
                    _ => (),
                }
            }
        }
        packages.insert(package.name.clone(), (package, Vec::new()));
    }

    // Get Packages.xz.
    let url = format!(
        "{}/dists/{}/{}/binary-{}/Packages.xz",
        archive_url, suite, target.component, target.arch
    );
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;

    // Decompress Packages.xz.
    let mut content = String::new();
    let mut decoder = XzDecoder::new_multi_decoder(&bytes[..]);
    decoder.read_to_string(&mut content)?;
    std::fs::write("Packages", content.clone())?;

    // Parse Packages entry to find binary package filenames.
    'block_iteration: for block in content.split("\n\n").filter(|block| !block.is_empty()) {
        let mut name = String::new();
        let mut version = String::new();
        let mut source = String::new();

        for line in block.lines() {
            if let Some((key, value)) = line.split_once(":") {
                match key {
                    "Package" => name = value.trim().to_string(),
                    "Version" => version = value.trim().to_string(),
                    "Source" => {
                        if let Some((source_package, _)) = packages.get_mut(value.trim()) {
                            source = source_package.name.clone();
                        } else {
                            anyhow::bail!(
                                "Found a binary package '{}' without an accompying source package '{}'.",
                                name,
                                value
                            );
                        }
                    }
                    "Filename" => {
                        println!("{} {} {}", name.clone(), source.clone(), value);

                        // Fetch .deb file.
                        let url = format!("{}/{}", archive_url, value.trim());
                        let response = client.get(url).send().await?.error_for_status()?;
                        let deb = response.bytes().await?.into();

                        // If there's no source field, assume the source is named the same as the binary package.
                        if source.is_empty() {
                            source = name.clone();
                        }

                        // Add binary package to it's source package.
                        let (_, binary_packages) = packages.get_mut(&source).unwrap();
                        binary_packages.push(BinaryPackage { name, version, deb });

                        continue 'block_iteration;
                    }
                    _ => (),
                }
            }
        }
    }

    Ok(packages.into_values().collect())
}
