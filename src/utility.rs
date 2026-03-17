use reqwest::Client;
use tokio::time::{Duration, sleep};

const MAX_ATTEMPTS: u32 = 5;

pub async fn get_with_retry(client: &Client, url: &str) -> anyhow::Result<reqwest::Response> {
    let mut attempts = 0;

    loop {
        attempts += 1;

        match client.get(url).send().await {
            Ok(response) => return Ok(response),
            Err(e) if e.is_timeout() && attempts < MAX_ATTEMPTS => {
                println!("Timeout. Retry {}.", attempts);
                sleep(Duration::from_secs(2u64.pow(attempts))).await
            }
            Err(e) => return Err(e.into()),
        }
    }
}
