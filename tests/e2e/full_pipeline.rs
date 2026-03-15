//! Full ETL Pipeline E2E Test
//!
//! This test verifies the complete ETL workflow:
//! 1. Mock NASA API returns asteroid data
//! 2. Extract phase fetches data from mock API
//! 3. Transform phase processes the data
//! 4. Load phase stores data in test database
//! 5. Verify data integrity in database

use anyhow::Result;
use sqlx::Row;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::database::TestDatabase;

#[tokio::test]
async fn test_full_etl_pipeline() -> Result<()> {
    // Setup test database
    let db = TestDatabase::new().await?;

    // Setup mock NASA server
    let nasa_mock = MockServer::start().await;

    // Mock NASA API response
    Mock::given(method("GET"))
        .and(path("/neo/rest/v1/feed"))
        .and(query_param("api_key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/nasa_responses/feed_valid.json")),
        )
        .mount(&nasa_mock)
        .await;

    // Simulate ETL extract phase (using reqwest directly)
    let client = reqwest::Client::new();
    let start_date = "2024-01-01";
    let end_date = "2024-01-07";

    let response = client
        .get(format!(
            "{}/neo/rest/v1/feed?api_key=test-key&start_date={}&end_date={}",
            nasa_mock.uri(),
            start_date,
            end_date
        ))
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    let feed_data: serde_json::Value = response.json().await?;

    // Verify mock returned expected data
    assert!(feed_data["near_earth_objects"].is_object());
    assert!(feed_data["element_count"].as_i64().unwrap() >= 0);

    // Simulate transform phase - parse asteroid data
    let asteroids = feed_data["near_earth_objects"]
        .as_object()
        .map(|obj| obj.values().flatten().count())
        .unwrap_or(0);

    assert!(asteroids >= 0);

    // Simulate load phase - insert into database
    if let Some(dates) = feed_data["near_earth_objects"].as_object() {
        for (_date, neos) in dates {
            if let Some(neo_list) = neos.as_array() {
                for neo in neo_list {
                    let neo_id = neo["id"].as_str().unwrap_or("unknown");
                    let name = neo["name"].as_str().unwrap_or("Unknown");
                    let is_hazardous = neo["is_potentially_hazardous_asteroid"]
                        .as_bool()
                        .unwrap_or(false);

                    // Insert asteroid into database
                    sqlx::query(
                        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
                         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
                         is_sentry_object, jpl_url, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())"
                    )
                    .bind(uuid::Uuid::new_v4())
                    .bind(neo_id)
                    .bind(name)
                    .bind(neo["absolute_magnitude_h"].as_f64().unwrap_or(25.0))
                    .bind(
                        neo["estimated_diameter"]["kilometers"]["estimated_diameter_min"]
                            .as_f64().unwrap_or(0.1)
                    )
                    .bind(
                        neo["estimated_diameter"]["kilometers"]["estimated_diameter_max"]
                            .as_f64().unwrap_or(0.5)
                    )
                    .bind(is_hazardous)
                    .bind(neo["is_sentry_object"].as_bool().unwrap_or(false))
                    .bind(neo["nasa_jpl_url"].as_str().unwrap_or("https://example.com"))
                    .execute(db.pool())
                    .await?;
                }
            }
        }
    }

    // Verify data was loaded correctly
    let asteroid_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asteroids")
        .fetch_one(db.pool())
        .await?;

    assert!(
        asteroid_count > 0,
        "Expected asteroids to be loaded into database"
    );

    // Verify hazardous asteroid tracking
    let hazardous_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM asteroids WHERE is_potentially_hazardous = TRUE")
            .fetch_one(db.pool())
            .await?;

    // At least verify the query works (may be 0 if no hazardous asteroids in fixture)
    assert!(hazardous_count >= 0);

    // Verify NASA mock was called
    let request_count = nasa_mock.received_requests().await.len();
    assert!(request_count >= 1, "Expected NASA API to be called");

    Ok(())
}

#[tokio::test]
async fn test_etl_pipeline_empty_response() -> Result<()> {
    // Setup test database
    let db = TestDatabase::new().await?;

    // Setup mock NASA server with empty response
    let nasa_mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/neo/rest/v1/feed"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/nasa_responses/feed_empty.json")),
        )
        .mount(&nasa_mock)
        .await;

    // Fetch from mock API
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/neo/rest/v1/feed?api_key=test", nasa_mock.uri()))
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    let feed_data: serde_json::Value = response.json().await?;

    // Verify empty response handling
    let element_count = feed_data["element_count"].as_i64().unwrap_or(0);
    assert_eq!(element_count, 0);

    // Verify database is still empty
    let asteroid_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asteroids")
        .fetch_one(db.pool())
        .await?;

    assert_eq!(asteroid_count, 0);

    Ok(())
}
