//! Example of connecting to the Sentinel API using Reqwest.
//!
//! Ensure the server is running (`cargo run serve`) before executing:
//! `cargo run --example api_client`

use reqwest::Result;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8080/api";

    println!("Checking Sentinel Health...");
    let res = client.get(format!("{}/health", base_url)).send().await?;
    let json: Value = res.json().await?;
    println!("{:#?}", json);

    println!("\nFetching Velocity Statistics (30d window)...");
    let res = client
        .get(format!("{}/velocity?period=30d", base_url))
        .send()
        .await?;
    let json: Value = res.json().await?;
    println!(
        "Found {} instances.",
        json["data"].as_array().unwrap().len()
    );

    Ok(())
}
