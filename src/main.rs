mod fetch;
mod utility;

use fetch::*;
use reqwest::Client;

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

    for package in packages {
        println!("{}_{}", package.name, package.version);
        if let Some(build_log) = fetch_build_log(&client, &target, package.clone()).await? {
            std::fs::write(
                format!("build_logs/{}_{}_build_log", package.name, package.version),
                build_log.clone(),
            )?;
        }
    }

    Ok(())
}

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
    dsc: Option<String>,
}
