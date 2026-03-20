mod buildlog;
mod elf;
mod flags;
mod package;
mod utility;

use buildlog::*;
use elf::*;
use flags::*;
use package::*;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct Target {
    series: String,
    pocket: String,
    component: String,
    arch: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let target = Target {
        series: "resolute".to_string(),
        pocket: "Release".to_string(),
        component: "main".to_string(),
        arch: "amd64".to_string(),
    };
    let client = Client::builder()
        .user_agent("ubuntu-buildflags-audit/0.1")
        .build()?;

    // Get build flags.
    let build_flags = get_build_flags()?;

    // Get packages.
    let packages: Vec<(SourcePackage, Vec<BinaryPackage>)> =
        fetch_packages(&client, &target).await?;

    //std::fs::create_dir("build_flags")?;
    for (source_package, binary_packages) in packages.into_iter().take(3) {
        println!("{}_{}", source_package.name, source_package.version);

        // Get build log.
        //if let Some(build_log) = fetch_build_log(&client, &target, &package).await? {
        //    std::fs::write(
        //        format!("build_logs/{}_{}_build_log", package.name, package.version),
        //        build_log.clone(),
        //    )?;
        //}

        // Get elf files.
        for binary_package in binary_packages {
            let elfs = extract_elfs_from_binary_package(&binary_package).await?;
            println!("{} elfs extracted", elfs.len());
            // Validate elf files.
            for (path, data) in elfs {
                println!("{}: {} bytes", path, data.len());
                let flags = detect_build_flags_from_elf(&data[..])?;
                println!("{flags:?}")
            }
        }
    }

    Ok(())
}
