use crate::{Target, package::SourcePackage, utility::get_with_retry};
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use std::io::Read;

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
    package: &SourcePackage,
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

                let response = get_with_retry(client, &build_log_url)
                    .await?
                    .error_for_status()?;
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
