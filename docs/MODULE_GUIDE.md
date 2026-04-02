# Module Development Guide

This guide outlines the module structure, design patterns, and best practices for the Rustroid Sentinel codebase.

## Code Organization Standards

### File Size Limits

| Metric                  | Target   | Enforcement                      |
| ----------------------- | -------- | -------------------------------- |
| Max function length     | 50 lines | Clippy lint `too_many_lines`     |
| Max function parameters | 5        | Clippy lint `too_many_arguments` |
| Max module depth        | 3 levels | Manual review                    |

### Module Structure Template

```
src/
├── lib.rs                  # Crate root, re-exports public API
├── main.rs                 # CLI entry point only
├── api/
│   ├── mod.rs              # Module root, re-exports
│   ├── handlers/           # One file per resource
│   │   ├── health.rs
│   │   ├── stats.rs
│   │   └── ...
│   ├── routes.rs           # Route registration
│   └── types.rs            # Shared types
├── database/
│   ├── mod.rs
│   ├── dashboard.rs        # Read operations
│   ├── repository.rs       # Write operations
│   └── error.rs            # Error types
└── utils/                  # Pure functions, helpers
```

## Module Design Patterns

### 1. Module Root Pattern (`mod.rs`)

Each module should have a `mod.rs` that:

- Declares submodules
- Re-exports public items for convenient access
- Contains module-level documentation

```rust
//! # Module Name
//!
//! Brief description of module purpose.

pub mod submodule;
pub mod error;

// Re-exports for convenient access
pub use submodule::PublicItem;
pub use error::Error;
```

### 2. Error Handling Pattern

Use `thiserror` for library errors and `anyhow` for application logic:

```rust
// library/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),

    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, MyError>;
```

```rust
// application logic
use anyhow::{Context, Result};

pub async fn do_something() -> Result<()> {
    some_operation()
        .await
        .context("operation failed")?;
    Ok(())
}
```

### 3. Handler Pattern (API Layer)

Each handler should:

- Be in its own file if >50 lines
- Use extractors for dependencies
- Return `impl IntoResponse` or typed response
- Log errors with context

```rust
// api/handlers/resource.rs
use axum::{Json, extract::State};
use tracing::{error, info};

use crate::api::types::ApiResponse;
use crate::server::AppState;

/// GET /api/resource
pub async fn get_resource(State(state): State<AppState>) -> Json<ApiResponse<Resource>> {
    info!("Resource requested");

    match fetch_resource(&state.db_pool).await {
        Ok(resource) => Json(ApiResponse::success(resource)),
        Err(e) => {
            error!("Failed to fetch resource: {}", e);
            Json(ApiResponse::error_message(e.to_string()))
        }
    }
}
```

### 4. Repository Pattern (Database Layer)

Separate read and write operations:

```rust
// database/dashboard.rs - Read operations
pub struct DashboardRepository;

impl DashboardRepository {
    pub async fn get_stats(pool: &PgPool) -> Result<DashboardStats, sqlx::Error> {
        // Read-only queries
    }
}

// database/repository.rs - Write operations
pub async fn upsert_batch(pool: &PgPool, items: Vec<Item>) -> Result<(), sqlx::Error> {
    // Write operations with transactions
}
```

## Testability Checklist

Before merging a module, ensure:

- [ ] **Pure functions extracted**: Business logic separated from I/O
- [ ] **Dependencies injected**: No hardcoded `Client::new()` or `connect()`
- [ ] **Error types defined**: Module has `error.rs` with structured errors
- [ ] **Tests included**: At least one test per public function
- [ ] **File size <300 lines**: Split if larger
- [ ] **Function size <50 lines**: Extract helpers if larger
- [ ] **Parameters ≤5**: Use struct builder if more

## Refactoring Triggers

Split a file when:

1. It exceeds 300 lines
2. It handles multiple responsibilities
3. Functions exceed 50 lines
4. You need to import it partially

Extract a module when:

1. Directory has >5 files
2. Files share common types/errors
3. Functionality is self-contained

## Example: Creating a New Handler

1. **Create handler file** (`src/api/handlers/new_feature.rs`):

```rust
use axum::{Json, extract::State};
use crate::server::AppState;
use crate::api::types::ApiResponse;

pub async fn get_feature(State(state): State<AppState>) -> Json<ApiResponse<Feature>> {
    // Implementation
}
```

2. **Add to module** (`src/api/handlers/mod.rs`):

```rust
pub mod new_feature;
pub use new_feature::get_feature;
```

3. **Add route** (`src/api/routes.rs`):

```rust
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/feature", get(get_feature))
        // ...
}
```

4. **Add tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_feature() {
        // Test implementation
    }
}
```

## Anti-Patterns to Avoid

| Pattern                   | Why It's Bad         | Fix                     |
| ------------------------- | -------------------- | ----------------------- |
| God files (>500 lines)    | Hard to navigate     | Split by responsibility |
| Functions with 10+ params | Hard to test         | Use builder pattern     |
| Hardcoded HTTP/DB calls   | Can't mock           | Inject via traits       |
| Mixed pure + impure logic | Can't test isolation | Extract pure functions  |
| Global state              | Tests interfere      | Pass state via structs  |

## Current Metrics

| Module          | Files | Avg Size | Max Size | Status |
| --------------- | ----- | -------- | -------- | ------ |
| `metrics/`      | 7     | 123 LOC  | 152 LOC  | Pass   |
| `server/`       | 5     | 82 LOC   | 109 LOC  | Pass   |
| `api/handlers/` | 7     | 74 LOC   | 238 LOC  | Pass   |

Target: All files <300 LOC
