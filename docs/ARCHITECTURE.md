# Architecture

Rustroid Sentinel is a modular monolith written in Rust, leveraging `tokio` for high-performance async operations.

## Components

1. **CLI Commands (`src/cli/`)**: Standalone tools that act as cron jobs or manual invocation scripts (`extract`, `transform`, `load`, `alert`, `serve`).
2. **NASA API Client (`src/nasa/`)**: Strictly typed `reqwest` client handling pagination and serialization of NeoWs JSON feeds.
3. **Database (`src/database/`)**: `sqlx` repository managing PostgreSQL connections. The queries are separated into `repository.rs` (writes/upserts) and `dashboard.rs` (reads/analytics).
4. **Web Server (`src/server.rs`, `src/api/`)**: `axum` based REST API protected by rate-limiting, serving cached HTML/JS bundles.
5. **Alerting (`src/alert/`)**: Simple HTTP webhook push mechanism for Discord integration.

## Design Decisions

- **Idempotency**: All ETL phases (`extract`, `transform`, `load`) can be re-run on the same inputs without causing data duplication, thanks to aggressive ON CONFLICT updates in PostgreSQL.
- **Fail-Fast**: The application will panic at startup if the configuration or database URL is malformed, preventing silent misconfigurations.
- **Observability Driven**: Every HTTP request emits structured trace logs and updates Prometheus counters.
