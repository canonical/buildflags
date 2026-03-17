mod fetch;
mod utility;

use fetch::*;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct Target {
    series: String,
    pocket: String,
    component: String,
    arch: String,
}

#[derive(Default, Debug, Clone)]
pub struct Package {
    name: String,
    version: String,
    directory: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let target = Target {
        series: "resolute".to_string(),
        pocket: "Proposed".to_string(),
        component: "main".to_string(),
        arch: "amd64".to_string(),
    };
    let client = Client::new();

    // Get packages.
    let packages = fetch_packages(&client, &target).await?;

    //std::fs::create_dir("build_flags")?;
    for package in packages {
        println!("{}_{}", package.name, package.version);

        // Get build log.
        //if let Some(build_log) = fetch_build_log(&client, &target, &package).await? {
        //    std::fs::write(
        //        format!("build_logs/{}_{}_build_log", package.name, package.version),
        //        build_log.clone(),
        //    )?;
        //}

        // Get elf files.
        let elfs = fetch_elfs(&client, &target, &package).await?;
        for (path, data) in elfs {
            println!("{}: {} bytes", path, data.len());
        }
    }

    Ok(())
}
