//! Complete ETL Pipeline Example
//!
//! Run using: `cargo run --example basic_etl`

use std::process::Command;

fn main() {
    println!("Starting the manual ETL pipeline sequence...");

    // Step 1: Extract
    println!("\n=> 1. Extracting data from NASA NeoWs API...");
    let extract_status = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            "--start-date",
            "2024-01-01",
            "--end-date",
            "2024-01-02",
        ])
        .status()
        .expect("Failed to execute extract");
    assert!(extract_status.success());

    // Step 2: Transform
    println!("\n=> 2. Transforming raw JSON to relational payloads...");
    let transform_status = Command::new("cargo")
        .args(["run", "--", "transform"])
        .status()
        .expect("Failed to execute transform");
    assert!(transform_status.success());

    // Step 3: Load
    println!("\n=> 3. Loading transformed models into PostgreSQL...");
    let load_status = Command::new("cargo")
        .args(["run", "--", "load"])
        .status()
        .expect("Failed to execute load");
    assert!(load_status.success());

    // Step 4: Alert
    println!("\n=> 4. Dispatching alerts to Discord...");
    let alert_status = Command::new("cargo")
        .args(["run", "--", "alert"])
        .status()
        .expect("Failed to execute alert");
    assert!(alert_status.success());

    println!("\nPipeline completed successfully.");
}
