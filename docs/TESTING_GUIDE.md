# Testing Guide for Rustroid Sentinel

This guide covers how to write, run, and maintain tests for the Rustroid Sentinel project.

## 📊 Test Structure

```
tests/
├── common/           # Shared test utilities
│   ├── mod.rs
│   ├── database.rs   # Test database setup (testcontainers)
│   ├── fixtures.rs   # Test data fixtures
│   └── mock_server.rs # Mock HTTP servers (wiremock)
├── integration/      # Integration tests
│   ├── mod.rs
│   ├── api/          # API endpoint tests
│   ├── database/     # Database operation tests
│   ├── etl/          # ETL pipeline tests
│   └── alerting/     # Alert system tests
├── e2e/              # End-to-end tests
│   ├── mod.rs
│   └── full_pipeline.rs
└── fixtures/         # Test data files
    └── nasa_responses/
```

## 🚀 Quick Start

### Run All Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration

# Run E2E tests (may be slow)
cargo test --test e2e
```

### Run Specific Tests

```bash
# Run tests matching a pattern
cargo test test_health

# Run tests in a specific module
cargo test --test integration api::health

# Run tests with output
cargo test -- --nocapture

# Run tests sequentially (for DB tests)
cargo test -- --test-threads=1
```

### Generate Coverage Report

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate HTML report
cargo llvm-cov --html --open

# Generate codecov report
cargo llvm-cov --codecov --output-path codecov.json
```

## 📝 Writing Tests

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name_success() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_value);
    }

    #[test]
    fn test_function_name_error_case() {
        // Arrange
        let invalid_input = create_invalid_input();
        
        // Act
        let result = function_under_test(invalid_input);
        
        // Assert
        assert!(result.is_err());
        assert!(matches!(result, Err(ErrorType::ExpectedVariant)));
    }
}
```

### Integration Test Template

```rust
use anyhow::Result;
use rustroid_sentinel::tests::common::{setup_test_database, MockNasaServer};

#[tokio::test]
async fn test_integration_scenario() -> Result<()> {
    // Setup
    let db = setup_test_database().await?;
    let mock_nasa = MockNasaServer::start().await?;
    
    // Configure mocks
    mock_nasa.mount_successful_feed("2024-01-01", "2024-01-07").await;
    
    // Seed database if needed
    seed_test_data(db.pool()).await?;
    
    // Act
    let result = function_under_test(db.pool(), &mock_nasa.uri()).await?;
    
    // Assert
    assert!(result.success);
    assert_eq!(result.processed_count, 10);
    
    Ok(())
}
```

### Using Fixtures

```rust
use crate::common::{load_nasa_fixture, NasaFixtureName, create_test_asteroid};

#[test]
fn test_with_fixture() {
    // Load NASA API fixture
    let fixture = load_nasa_fixture(NasaFixtureName::FeedValid);
    let parsed: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    
    // Use test data creators
    let asteroid = create_test_asteroid();
    assert_eq!(asteroid.name, "Test Asteroid");
}
```

## 🛠️ Test Utilities

### Database Testing

```rust
use rustroid_sentinel::tests::common::TestDatabase;

#[tokio::test]
async fn test_with_database() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;
    
    // Use db.pool() for queries
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asteroids")
        .fetch_one(db.pool())
        .await?;
    
    assert_eq!(count, 0);
    Ok(())
}
```

### HTTP Mocking

```rust
use rustroid_sentinel::tests::common::MockNasaServer;

#[tokio::test]
async fn test_with_mock_http() -> Result<()> {
    let mock = MockNasaServer::start().await?;
    
    // Configure response
    mock.mount_successful_feed("2024-01-01", "2024-01-07").await;
    
    // Make request
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/neo/rest/v1/feed", mock.uri()))
        .send()
        .await?;
    
    // Verify
    assert_eq!(response.status(), 200);
    assert_eq!(mock.received_requests().await, 1);
    
    Ok(())
}
```

## 📋 Test Naming Conventions

| Pattern | Example | Description |
|---------|---------|-------------|
| `test_<function>_<scenario>_<expected>` | `test_parse_velocity_valid_format_returns_ok` | Unit tests |
| `test_<endpoint>_<scenario>` | `test_health_endpoint_returns_200` | API tests |
| `test_<operation>_<condition>` | `test_get_stats_empty_database` | DB tests |

## ✅ Test Quality Checklist

Before submitting a PR with tests:

- [ ] Test names follow naming conventions
- [ ] Tests use Arrange-Act-Assert pattern
- [ ] Tests are independent (no shared state)
- [ ] Error paths are tested, not just success
- [ ] Edge cases covered (empty input, max values, invalid data)
- [ ] No hardcoded timeouts (use mock time or configurable)
- [ ] No `println!` in passing tests (use `tracing` for debugging)
- [ ] Tests pass with `--test-threads=1` (for DB tests)

## 🎯 Coverage Goals

| Module | Target | Critical Paths |
|--------|--------|----------------|
| `src/nasa/` | 90% | Error handling, retry logic |
| `src/database/` | 85% | All queries, transactions |
| `src/api/` | 80% | All endpoints, error responses |
| `src/alert/` | 90% | Threshold logic, webhooks |
| `src/etl/` | 85% | Transform functions |
| `src/config/` | 95% | All parsing paths |
| `src/models/` | 80% | Validation, serialization |

## 🚫 Anti-Patterns

```rust
// ❌ BAD: Tests depending on order
#[tokio::test]
async fn test_first() { /* sets state */ }

#[tokio::test]
async fn test_second() { /* depends on first */ }

// ✅ GOOD: Independent tests
#[tokio::test]
async fn test_with_own_setup() {
    let db = setup_test_database().await?;
    // Self-contained test
}
```

```rust
// ❌ BAD: Hardcoded sleep
tokio::time::sleep(Duration::from_secs(5)).await;

// ✅ GOOD: Mock time or instant completion
// Use mock servers that respond immediately
```

```rust
// ❌ BAD: Shared global state
static mut GLOBAL_DB: Option<PgPool> = None;

// ✅ GOOD: Per-test isolation
let db = TestDatabase::new().await?;
```

## 🔧 Troubleshooting

### Test Fails with "Connection Refused"

Ensure testcontainers is running and Docker daemon is accessible:

```bash
docker ps  # Should show test containers
```

### Test Hangs

- Check for unclosed database connections
- Ensure mock servers have expected responses mounted
- Run with timeout: `timeout 30s cargo test`

### Flaky Tests

- Ensure no shared state between tests
- Use transactions or DROP DATABASE for cleanup
- Run with `--test-threads=1` for DB tests

## 📚 Additional Resources

- [Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [tokio - Testing async code](https://tokio.rs/tokio/tutorial/testing)
- [wiremock docs](https://docs.rs/wiremock/latest/wiremock/)
- [testcontainers docs](https://docs.rs/testcontainers/latest/testcontainers/)
