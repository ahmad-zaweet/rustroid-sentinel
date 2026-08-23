![Rustroid Sentinel Cover Image](assets/rustroid-sentinel-cover.png)

# Rustroid Sentinel

[![pipeline status](https://gitlab.com/AhmadZaweet/rustroid-sentinel/badges/master/pipeline.svg)](https://gitlab.com/AhmadZaweet/rustroid-sentinel/-/commits/master)

![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg?style=flat&logo=rust&logoColor=white)
![Axum](https://img.shields.io/badge/axum-%23E0234E.svg?style=flat)
![Tokio](https://img.shields.io/badge/tokio-%23007ACC.svg?style=flat&logo=tokio)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-316192?style=flat&logo=postgresql&logoColor=white)
![SQLx](https://img.shields.io/badge/sqlx-%23000000.svg?style=flat&logo=&logoColor=white)
![OpenTelemetry](https://img.shields.io/badge/OpenTelemetry-%23000000.svg?style=flat&logo=opentelemetry&logoColor=white)

[![Discord](https://img.shields.io/badge/Discord-%235865F2.svg?style=flat&logo=discord&logoColor=white)](https://discord.gg/GHT55B3Mdp)
[![Live Deployment](https://img.shields.io/badge/Render-Live-success?style=flat&logo=render&logoColor=white)](https://rustroid-sentinel.onrender.com/)

## Table of Contents

- [Overview](#-overview)
- [Discord Community](#-discord-community)
- [Key Features](#-key-features)
- [Architecture](#-architecture)
  - [Request Lifecycle](#request-lifecycle)
  - [Data Model](#data-model)
  - [Error Handling](#error-handling)
- [Project Structure](#-project-structure)
- [Environment Variables](#-environment-variables)
- [Getting Started](#-getting-started)
  - [Prerequisites](#-prerequisites)
  - [Local Development Setup](#-local-development-setup)
  - [Running with Docker](#-running-with-docker)
  - [Running the Examples](#-running-the-examples)
- [CLI Commands](#-cli-commands)
- [Rate Limiting & Security](#-rate-limiting--security)
- [Health Checks](#-health-checks)
- [Observability](#-observability)
- [CI/CD Pipeline](#-cicd-pipeline)
- [Testing](#-testing)
- [Contributing](#-contributing)
- [License](#-license)
- [Acknowledgments](#-acknowledgments)

---

## 📖 Overview

Rustroid Sentinel is a Rust/Tokio platform that turns NASA's raw near-Earth-object feed into a queryable, alertable, self-governing asteroid intelligence system: it ingests NEO approach data idempotently, enriches every catalog object with real JPL impact-risk and orbital data, serves it through an Axum API and HTMX dashboard (including a live SSE hazard stream and pgvector similarity search), and keeps its own database, compute, and alert volume bounded without a human watching it. It's built the way a system that has to run unattended on a metered, size-capped database has to be built — every write is safe to retry, every external dependency degrades instead of taking the system down with it, and every secret is structurally incapable of ending up in a log line. None of that is incidental scaffolding around a simple fetch-and-alert script; it *is* the engineering.

**Tech stack:** Rust (Tokio, Axum, SQLx), PostgreSQL 17 (+ `pgvector`, `pg_trgm`), Askama + HTMX (server-rendered, no SPA framework) with a basecoat-ui + Tailwind CLI frontend build, Serenity-style Discord webhooks, Prometheus + OpenTelemetry/OTLP, Docker (`cargo-chef` multi-stage build), GitLab CI.

## 💬 Discord Community

Join the official [Rustroid Sentinel Discord Server](https://discord.gg/GHT55B3Mdp) to connect with fellow developers, discuss space data pipelines, and receive real-time hazard alerts directly.

## 🎯 Key Features

Grouped by what each cluster of decisions actually buys, not by which module it lives in.

**Idempotent, restart-safe ingestion.** A crashed or re-triggered pipeline run can never duplicate data or double-fire an alert — every write path is designed to be safely re-run, not just "usually fine to re-run."

- **Deterministic UUIDv5 identity** — asteroid/approach IDs are derived via `Uuid::new_v5()` from NASA's own natural keys, so a repeated load is a same-ID `INSERT ... ON CONFLICT`, not a check-then-write race that can duplicate rows under concurrent or retried runs ([`src/models/asteroid.rs`](src/models/asteroid.rs), [`src/models/approach.rs`](src/models/approach.rs)).
- **Streaming ETL, memory bounded by design, not by luck** — the extractor streams the NASA feed response straight to a temp file rather than buffering it (`NeoWsApi::get_feed`, [`src/nasa/asteroid_neows/api.rs`](src/nasa/asteroid_neows/api.rs)), and the loader reads NDJSON back line-by-line in 1,000-row batches ([`src/cli/load.rs`](src/cli/load.rs)) — so pipeline memory use doesn't scale with feed size.
- **Bulk UPSERT via `UNNEST`** — asteroid/approach batches write as a single `INSERT ... SELECT * FROM UNNEST($1::uuid[], ...)` per chunk inside one transaction instead of row-by-row inserts, turning an N-round-trip write into one ([`src/database/repository.rs`](src/database/repository.rs)).
- **Idempotent alerting** — a `LEFT JOIN alerts ... WHERE al.id IS NULL` query, backed by a `UNIQUE (approach_id, alert_type)` constraint, guarantees an approach is alerted at most once per channel even if the alert run itself crashes mid-batch and gets retried ([`src/alert/service.rs`](src/alert/service.rs), [`migrations/002_create_alerts_table.sql`](migrations/002_create_alerts_table.sql)).
- **Generated column for average diameter** — `estimated_diameter_avg_km` is `GENERATED ALWAYS AS ((min+max)/2.0) STORED`, so the average can never drift out of sync with `min`/`max` the way an application-computed copy could ([`migrations/003_add_diameter_avg_column.sql`](migrations/003_add_diameter_avg_column.sql)).
- **Rule-based hazard classification** — a pure function scores each approach Critical/High/Medium/Low from PHA designation, diameter, velocity, and miss distance, independently unit-tested against every threshold boundary, so classification is deterministic and auditable rather than an opaque scoring blob ([`src/transform/mod.rs`](src/transform/mod.rs)).

**Defense-in-depth API layer.** No single control is load-bearing on its own — rate limiting, CSP, CORS, and body caps each independently bound a different failure mode, `tower`-composed in [`src/server/router.rs`](src/server/router.rs).

- **Per-IP rate limiting via `axum-governor`** — a `GovernorLayer` with a `PeerIp` extractor enforces `rate_limit_requests` per `rate_limit_period_seconds` (config-driven, default 100/60s), returning `429` once exhausted.
- **CSP + security headers via `axum-helmet`** — an explicit `Content-Security-Policy` allowlist (self + named CDNs) rather than a wildcard, so an injected script tag has nowhere it's allowed to load from.
- **Hand-rolled CORS allowlist** — `cors_middleware` only allows `localhost:8000`/`127.0.0.1:8000` origins (or no `Origin` header) and `GET` only; anything else gets `403` ([`src/server/middleware.rs`](src/server/middleware.rs)).
- **1 MiB request body cap** via `RequestBodyLimitLayer`, so an oversized request can't be used to exhaust memory before any handler logic runs.
- **Constant-time-authenticated internal webhook** — `POST /internal/events` compares `X-Internal-Token` against `INTERNAL_EVENT_TOKEN` in constant time (not a short-circuiting `==`, which would leak timing information about how much of the token matched), sits outside the public `/api` router, and carries its own tighter rate-limit bucket and 64 KB body cap ([`src/api/handlers/internal_events.rs`](src/api/handlers/internal_events.rs)).
- **SSE subscriber cap** — `GET /api/events/hazards` returns `503` once `max_hazard_subscribers` concurrent streams are held, since each open stream is a live server task that would otherwise grow unbounded with client count ([`src/api/handlers/hazard_events_sse.rs`](src/api/handlers/hazard_events_sse.rs)).
- **Retrying, TLS-strict outbound HTTP client** — `reqwest-retry`'s `ExponentialBackoff` wraps every outbound call, certificate/hostname validation is explicitly enforced (`danger_accept_invalid_certs(false)`), and redirects are capped at 5, so a flaky or malicious upstream can't silently downgrade the connection or trigger an unbounded redirect chain ([`src/api/client.rs`](src/api/client.rs)).
- **Differentiated cache-control policy** — API routes get `no-store, no-cache, must-revalidate`, static assets get `public, max-age=3600`, as two separate layers rather than one blanket policy that would either stale-cache live data or refetch immutable assets on every request ([`src/server/middleware.rs`](src/server/middleware.rs)).

**Structured error handling.** Five separate `thiserror` enums — `ApiError`, `AlertError`, `DatabaseError`, `NasaApiError`, `MetricsError` (one per domain module) — each expose `is_retryable()` so calling code can decide "retry this" vs. "give up" without string-matching an error message, and `ApiError` additionally maps to a `status_code()` used by its `IntoResponse` impl. Every HTTP error response, success or failure, is wrapped in the same `ApiResponse<T> { success, data, error }` JSON envelope, so API clients parse one shape instead of branching on status code first (see [Error Handling](#error-handling) below for the full mapping table).

**Secrets that can't leak.** `DatabaseConfig`, `NasaConfig`, `DiscordConfig`, `PrometheusConfig`, and `GrafanaCloudPrometheusConfig` each hand-implement `Debug` to redact URLs/keys/tokens — this isn't a "remember not to log the config" convention, it's structural: `info!(config = ?settings, ...)` in [`src/main.rs`](src/main.rs) physically cannot print a credential, because the redacted `Debug` impl is the only one that exists.

**Observability that degrades gracefully.** No single metrics backend being down takes visibility to zero.

- **Dual metrics pipeline, three-tier fallback** — a Prometheus registry is scraped at `/metrics`, and the same process pushes OTLP metrics to Grafana Cloud every 10 seconds. The dashboard's live-metrics widget queries Grafana Cloud Prometheus first, falls back to a legacy `query_url`, then falls back to database-derived counts if neither is configured — metrics degrade in fidelity, they don't go dark ([`src/metrics/otlp.rs`](src/metrics/otlp.rs), [`src/metrics/mod.rs`](src/metrics/mod.rs)).
- **OpenTelemetry distributed tracing** — `tracing` + `tracing-subscriber` with `EnvFilter`, and a `TraceLayer` on every HTTP request, so a slow or failing request can be traced end-to-end rather than reconstructed from scattered log lines.
- **HTMX partial SSR** — the dashboard's table, ETL history, velocity chart, and metrics widget are independently refreshable Askama templates served from dedicated `/dashboard/*` endpoints, so a slow metrics query doesn't block the rest of the page from rendering ([`src/api/handlers/dashboard.rs`](src/api/handlers/dashboard.rs)).

**Catalog intelligence & enrichment.** Beyond ingest-and-alert, the system builds a queryable picture of the whole NEO catalog, not just the approaches that triggered an alert.

- **Live SSE hazard stream** — `GET /api/events/hazards` fans out newly-loaded Critical/High approaches over Server-Sent Events with a 15s keep-alive and a typed `lagged` event (with skip count) for subscribers that fall behind, instead of a silent gap; fed by an internal webhook rather than Postgres `LISTEN/NOTIFY` specifically so it survives a pooled (PgBouncer transaction-mode) database connection where `LISTEN` isn't available ([`src/events/`](src/events/), [`docs/NEON_SERVERLESS_PLAN.md`](docs/NEON_SERVERLESS_PLAN.md)).
- **Real Torino/Palermo hazard scoring via JPL Sentry** — `sentry` checks every `is_sentry_object` asteroid against JPL's Sentry impact-monitoring API and stores actual `torino_scale`/`palermo_scale` values, not a locally-approximated score, incremental via `sentry_checked_at` ([`src/nasa/jpl_sentry/`](src/nasa/jpl_sentry/), [`migrations/005_add_sentry_hazard_scales.sql`](migrations/005_add_sentry_hazard_scales.sql)). Those scores surface directly in Discord alert embeds too — Torino is always shown, Palermo appears as a trailing field when the asteroid has one ([`src/alert/discord.rs`](src/alert/discord.rs)).
- **Orbital-element enrichment via JPL SBDB** — `orbits` fetches eccentricity, semi-major axis, inclination, orbit class, spectral class, and albedo for every asteroid, incremental via `orbit_checked_at`, with a consecutive-failure abort so a throttled/blocked run stops instead of escalating into a hard block ([`src/nasa/jpl_sbdb/`](src/nasa/jpl_sbdb/), [`migrations/006_add_asteroid_orbits.sql`](migrations/006_add_asteroid_orbits.sql)).
- **Browsable, searchable catalog** — `GET /asteroids` / `GET /asteroids/{neo_reference_id}` serve keyset-paginated (opaque cursor, no `OFFSET`) list and detail views with filters across hazard class, PHA flag, date/diameter range, orbit class, spectral class, albedo, and Torino/Palermo minimums, plus trigram (`pg_trgm`) name search ([`src/api/handlers/catalog.rs`](src/api/handlers/catalog.rs), [`migrations/007_pg_trgm_name_index.sql`](migrations/007_pg_trgm_name_index.sql)). The main dashboard table has its own lighter-weight `sentry_only` toggle (`GET /dashboard/table?sentry_only=true`) that filters to asteroids with a non-null Torino/Palermo score ([`src/api/handlers/dashboard.rs`](src/api/handlers/dashboard.rs)).
- **pgvector similarity search** — `vectorize` computes a normalized 16-dimension feature embedding per asteroid into an HNSW-indexed table; `GET /api/asteroids/{id}/similar` does a read-only nearest-neighbor lookup, with the index rebuilt only by the batch job, never on request ([`src/transform/embedding.rs`](src/transform/embedding.rs), [`migrations/008_add_asteroid_embeddings.sql`](migrations/008_add_asteroid_embeddings.sql)).
- **Storage-budget-aware retention** — `prune` deletes `approaches`/`etl_events` rows past a configurable age (with a minimum-row floor so the dashboard is never empty), and the metrics widget tracks `pg_database_size` against a configurable budget with a warn/critical gauge ([`src/database/retention.rs`](src/database/retention.rs), [`src/cli/prune.rs`](src/cli/prune.rs)).
- **Weekly Discord report** — `report` aggregates the trailing 7 days of approaches into a summary embed through the same idempotent Discord client used for individual alerts ([`src/cli/report.rs`](src/cli/report.rs)); the same summary is also served as an HTMX dashboard partial at `GET /dashboard/report` ([`src/api/handlers/dashboard.rs`](src/api/handlers/dashboard.rs) `refresh_weekly_report`).
- **One batched `pipeline` command** — `extract → transform → load → prune → vectorize → report` (report Sundays only, or `--force-report`) runs in a single process, so a compute-metered database that scales to zero wakes once per scheduled run instead of once per stage ([`src/cli/pipeline.rs`](src/cli/pipeline.rs)).

**Build, config & release engineering.**

- **Compile-time feature isolation** — `api`, `alerting`, `metrics`, and `etl` are Cargo features gating entire modules and CLI subcommands (`#[cfg(feature = "...")]`), so a metrics-only or alerting-only binary builds without pulling in Axum or Serenity — four independently-buildable binaries from one codebase ([`src/lib.rs`](src/lib.rs), [`src/main.rs`](src/main.rs)).
- **Layered configuration** — `config/config.toml` (optional base) → `config/{RUN_ENV}.toml` (required) → `SERVICE__`-prefixed environment variables, merged with typed deserialization errors rather than silently falling back on a malformed value ([`src/settings.rs`](src/settings.rs)).
- **Graceful shutdown** — `tokio::select!` over `SIGINT`/`SIGTERM` with `axum::serve(...).with_graceful_shutdown(...)`; if signal-handler installation itself fails, it logs and blocks forever rather than silently exiting mid-request ([`src/server/shutdown.rs`](src/server/shutdown.rs)). A `tokio::sync::watch` channel on `AppState.shutdown` is fanned out to the SSE hazard stream, which `select!`s on `shutdown.changed()` so an otherwise-infinite stream ends immediately instead of leaving graceful shutdown waiting on a client that never disconnects ([`src/api/handlers/hazard_events_sse.rs`](src/api/handlers/hazard_events_sse.rs)).
- **Multi-stage, layer-cached Docker build** — `cargo-chef` separates the dependency-compile layer from the source-compile layer so a source-only change doesn't re-download and re-compile every dependency; runtime stage is a minimal Alpine image running as a non-root `sentinel` user with a container-level `HEALTHCHECK` hitting `/api/health` ([`Dockerfile`](Dockerfile)).
- **basecoat-ui + Tailwind CLI frontend build** — the dashboard compiles Tailwind via `@tailwindcss/cli` (`static/css/input.css` → `dist.css`) instead of a CDN `<script>` tag, adopts basecoat-ui's shadcn-style component classes on top of the existing Askama + HTMX server-rendered stack (no SPA framework), and uses the real Rustroid Sentinel logo mark across header, favicon, and welcome banner instead of a generic icon placeholder ([`package.json`](package.json), [`templates/base.html`](templates/base.html)).
- **Tests are real, not yet CI-gated** — `wiremock` mocks the NASA API, `bollard` spins up disposable Postgres containers for integration tests, and `cargo-deny` enforces license/advisory policy — all three run locally and via `.pre-commit-config.yaml`, but none is currently a required GitLab CI check yet; that's a known, deliberate gap, not an oversight (see [Testing](#-testing) and [`deny.toml`](deny.toml)).

## 🏗 Architecture

### Request Lifecycle

The diagram below mirrors the layer declaration order in [`src/server/router.rs`](src/server/router.rs):

```mermaid
flowchart TD
    Client([Client]) --> RealIp["RealIpLayer<br/>resolves true client IP"]
    RealIp --> Governor["GovernorLayer (axum-governor)<br/>per-IP quota: rate_limit_requests / rate_limit_period_seconds"]
    Governor -->|quota exceeded| R429["429 Too Many Requests"]
    Governor --> Routes{Route match}
    Routes -->|GET /| Dashboard["render_dashboard<br/>Askama SSR"]
    Routes -->|GET /health| Health["health_check_handler<br/>SELECT 1 -> 200 / 503"]
    Routes -->|/api/*| ApiRouter["api_router<br/>+ api_cache_control: no-store<br/>(incl. GET /api/events/hazards SSE)"]
    Routes -->|/dashboard/*| DashRouter["dashboard_router<br/>HTMX partials"]
    Routes -->|POST /internal/events| Internal["ingest_events<br/>X-Internal-Token (constant-time) + tighter GovernorLayer + 64KB cap"]
    Routes -->|GET /metrics| Prom["Prometheus text export"]
    Routes -->|fallback| Static["ServeDir static/<br/>+ static_cache_control: 1h"]
    Dashboard & Health & ApiRouter & DashRouter & Internal & Prom & Static --> Body["RequestBodyLimitLayer<br/>1 MiB cap"]
    Body --> Helmet["axum-helmet<br/>CSP + security headers"]
    Helmet --> Trace["TraceLayer"]
    Trace --> Gzip["CompressionLayer (gzip)"]
    Gzip --> Metrics["metrics_middleware<br/>records Prometheus + OTLP timers"]
    Metrics --> Cors["cors_middleware<br/>origin allowlist: localhost:8000 only"]
    Cors -->|origin rejected| R403["403 Forbidden"]
    Cors --> Timeout["TimeoutLayer<br/>request_timeout_seconds"]
    Timeout -->|exceeded| R504["504 Gateway Timeout"]
    Timeout --> Response([Response])
```

### Data Model

```mermaid
erDiagram
    ASTEROIDS ||--o{ APPROACHES : "has many"
    APPROACHES ||--o{ ALERTS : "has many (per alert_type)"
    ASTEROIDS ||--o| ASTEROID_ORBITS : "has one"
    ASTEROIDS ||--o| ASTEROID_EMBEDDINGS : "has one"

    ASTEROIDS {
        uuid id PK "UUIDv5(namespace, neo_reference_id) - deterministic"
        text neo_reference_id UK "NASA natural key"
        text name
        float8 absolute_magnitude
        float8 estimated_diameter_min_km
        float8 estimated_diameter_max_km
        float8 estimated_diameter_avg_km "GENERATED ALWAYS AS (min+max)/2 STORED"
        bool is_potentially_hazardous
        bool is_sentry_object
        int2 torino_scale "from JPL Sentry API"
        float8 palermo_scale "from JPL Sentry API"
        timestamptz sentry_checked_at "staleness marker for `sentry` command"
        text nasa_jpl_url
        timestamptz created_at
        timestamptz updated_at
    }

    ASTEROID_ORBITS {
        uuid asteroid_id PK, FK
        float8 eccentricity
        float8 semi_major_axis_au
        float8 inclination_deg
        float8 ascending_node_deg
        float8 perihelion_arg_deg
        float8 mean_anomaly_deg
        float8 orbital_period_days
        text orbit_class
        text spectral_class
        float8 albedo
        timestamptz orbit_checked_at "staleness marker for `orbits` command"
    }

    ASTEROID_EMBEDDINGS {
        uuid asteroid_id PK, FK
        vector_16 embedding "pgvector, HNSW-indexed"
        timestamptz computed_at
    }

    APPROACHES {
        uuid id PK "UUIDv5(namespace, neo_reference_id:epoch)"
        uuid asteroid_id FK
        date close_approach_date
        int8 epoch_date_close_approach
        float8 velocity_km_per_s
        float8 velocity_km_per_h
        float8 miss_distance_km
        float8 miss_distance_astronomical
        float8 miss_distance_lunar
        text orbiting_body
        text hazard_classification "Critical | High | Medium | Low"
        timestamptz created_at
    }

    ALERTS {
        uuid id PK
        uuid approach_id FK
        text alert_type
        timestamptz alerted_at
        jsonb payload
    }

    ETL_EVENTS {
        uuid id PK
        text source_file UK
        timestamptz started_at
        timestamptz completed_at
        text status "running | completed | failed"
        int4 asteroids_processed
        int4 approaches_processed
        text error_message
    }
```

`APPROACHES` has a `UNIQUE (asteroid_id, epoch_date_close_approach)` constraint so re-running the loader on the same source data is a no-op (`ON CONFLICT ... DO NOTHING`). `ALERTS` has `UNIQUE (approach_id, alert_type)` for the same reason. `ETL_EVENTS` tracks pipeline runs by `source_file` and is not foreign-keyed to the other tables — it's an audit log, not a domain relation. Indexes exist on `neo_reference_id`, `is_potentially_hazardous`, `asteroid_id`, `close_approach_date`, and `hazard_classification` ([`migrations/001_create_tables.sql`](migrations/001_create_tables.sql)).

`ASTEROID_ORBITS` and `ASTEROID_EMBEDDINGS` are one-row-per-asteroid enrichment tables, each `asteroid_id`-keyed and cascade-deleted with their parent, populated by the batch `orbits`/`vectorize` commands rather than the NeoWs ETL path ([`migrations/006_add_asteroid_orbits.sql`](migrations/006_add_asteroid_orbits.sql), [`migrations/008_add_asteroid_embeddings.sql`](migrations/008_add_asteroid_embeddings.sql)). `ASTEROID_EMBEDDINGS.embedding` is indexed with HNSW (`vector_l2_ops`), rebuilt only by `vectorize`, never on request.

Hazard events (`Critical`/`High` approaches, published to the SSE stream) are not a table — the `INSERT ... ON CONFLICT ... RETURNING id` in `AsteroidRepository::upsert_batch` tells the loader exactly which approach rows were newly inserted (not deduped), and those are turned into ephemeral `HazardEvent`s fanned out over an in-process `tokio::sync::broadcast` channel ([`src/events/mod.rs`](src/events/mod.rs)); the database row remains the source of truth, the event is just a low-latency notification.

### Error Handling

Each module owns a `thiserror` enum ([`src/api/error.rs`](src/api/error.rs), [`src/alert/error.rs`](src/alert/error.rs), [`src/database/error.rs`](src/database/error.rs), [`src/nasa/error.rs`](src/nasa/error.rs), [`src/metrics/error.rs`](src/metrics/error.rs)), each exposing `is_retryable()` and, for `ApiError`, a `status_code()` mapping used by its `IntoResponse` impl. Application-level code (CLI commands, `main.rs`) uses `anyhow::Result` with `.context()` instead of propagating the typed enums directly. `ApiError` maps to HTTP status as:

| `ApiError` variant | HTTP Status |
| --- | --- |
| `NotFound` | 404 |
| `InvalidQuery`, `InvalidBody` | 400 |
| `Unauthorized` | 401 |
| `Forbidden` | 403 |
| `Timeout` | 504 |
| `Unavailable` | 503 |
| `Database`, `Serialization`, `Internal`, `Metrics` | 500 |

Every `ApiError` response is wrapped in the same `ApiResponse<T> { success, data, error }` JSON envelope used by successful responses ([`src/api/types.rs`](src/api/types.rs)).

## 📂 Project Structure

```text
rustroid-sentinel/
├── src/
│   ├── lib.rs                  # Public library interface; feature-gated module re-exports
│   ├── main.rs                 # CLI entry point; dispatches all 11 subcommands
│   ├── api/                    # Axum handlers, routes, DTOs, Askama templates, HTTP client
│   │   ├── handlers/           # One handler module per endpoint (stats, velocity, approaches, etl_runs, health, dashboard, catalog, hazard_events_sse, internal_events)
│   │   └── cursor.rs           # Opaque base64 keyset-pagination cursor for the catalog list endpoint
│   ├── alert/                  # Hazard-alert dispatch: Discord webhook client + idempotent alert service
│   ├── cli/                    # extract / transform / load / alert / prune / sentry / orbits / vectorize / report / pipeline subcommand implementations
│   ├── database/                # Connection pool, migrations runner, write repository, read (dashboard) repository, retention/pruning, catalog queries, pgvector embeddings, weekly-report aggregation
│   ├── events/                  # HazardEvent + broadcast channel; optional pg-listen NOTIFY forwarder
│   ├── metrics/                 # Prometheus registry, OTLP exporter, Axum metrics middleware, Grafana Cloud query client, storage-budget gauge
│   ├── models/                  # Asteroid / Approach domain structs, hazard classification enum
│   ├── nasa/                    # Typed API clients + response DTOs: NeoWs (feed), JPL Sentry (Torino/Palermo), JPL SBDB (orbital elements)
│   ├── server/                  # Router assembly, middleware, shared AppState, graceful shutdown
│   ├── settings.rs              # Layered config loading (files + SERVICE__ env vars)
│   ├── transform/               # NASA DTO -> domain model conversion, hazard classification rules, pgvector feature-embedding normalization
│   └── error.rs                 # Top-level Error enum, re-exports module error types
├── migrations/                  # Raw SQL migrations, executed in order via sqlx::raw_sql on startup
├── static/
│   ├── css/                    # input.css (Tailwind + basecoat-ui source) -> dist.css (built output, npm run build:css)
│   ├── img/                    # Logo mark, favicons, apple-touch-icon (derived from assets/rustroid-sentinel-cover.png)
│   └── js/                     # Dashboard/interactions/welcome scripts, served via tower-http ServeDir
├── templates/                   # Askama templates (full page + HTMX partials), incl. dashboard/catalog.html, catalog-detail.html, partials/weekly-report.html
├── scripts/                     # One-off tooling, e.g. extract_logo_assets.py (crops the cover image into header/favicon sizes)
├── tests/                       # Integration/e2e tests: wiremock (NASA API), bollard (disposable Postgres)
├── examples/                    # Standalone runnable examples (api_client, basic_etl, custom_alert)
├── .gitlab/ci/                  # Split CI job definitions included by .gitlab-ci.yml (quality, docker build, ETL pipeline, weekly report)
├── Dockerfile                   # cargo-chef multi-stage build -> Alpine runtime
├── docker-compose.yml           # App + Postgres for local Docker Compose runs
├── deny.toml                    # cargo-deny license/advisory policy (run manually; not yet wired into CI)
├── package.json                 # Frontend build pipeline: @tailwindcss/cli build:css / watch:css scripts
└── .env.example                 # Environment variable scaffolding
```

## ⚙️ Environment Variables

Configuration is loaded by [`src/settings.rs`](src/settings.rs): `config/config.toml` (optional) → `config/{RUN_ENV}.toml` (required) → environment variables prefixed `SERVICE__` with `__` as the nesting separator. `RUN_ENV` itself is read directly (not `SERVICE__`-prefixed) and defaults to `development`.

Fields without a `#[serde(default)]` in the corresponding config struct are **required** — they must come from a TOML file or the environment, or startup fails with a `ServiceConfigError::Deserialize`.

**Database** (`SERVICE__DATABASE__…`)

| Variable | Required | Description |
| --- | --- | --- |
| `SERVICE__DATABASE__URL` | Yes | PostgreSQL connection string (pooled endpoint, e.g. Neon's `-pooler`) |
| `SERVICE__DATABASE__LISTEN_URL` | No | Direct (non-pooler) connection string, required only by the `pg-listen` feature — PgBouncer transaction-mode pooling doesn't support `LISTEN` |
| `SERVICE__DATABASE__MAX_CONNECTIONS` | Yes | Max pool size |
| `SERVICE__DATABASE__MIN_CONNECTIONS` | Yes | Min idle connections |
| `SERVICE__DATABASE__CONNECT_TIMEOUT_SECONDS` | Yes | Pool acquire timeout |

**NASA NeoWs client** (`SERVICE__NASA__…`)

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `SERVICE__NASA__API_KEY` | Yes | — | NASA Open API key (get one at api.nasa.gov) |
| `SERVICE__NASA__BASE_URL` | Yes | — | NASA API base URL |
| `SERVICE__NASA__TIMEOUT_SECONDS` | Yes | — | Per-request timeout |
| `SERVICE__NASA__MAX_RETRIES` | Yes | — | Exponential-backoff retry count |
| `SERVICE__NASA__RETRY_DELAY_MS` | Yes | — | Max backoff delay |
| `SERVICE__NASA__MAX_CONCURRENT_REQUESTS` | No | `5` | Semaphore-bounded concurrent NASA requests |

**Discord alerting** (`SERVICE__DISCORD__…`)

| Variable | Required | Description |
| --- | --- | --- |
| `SERVICE__DISCORD__WEBHOOK_URL` | Yes (may be empty) | Webhook URL; an empty string makes `DiscordClient::send_alert` a no-op rather than an error |
| `SERVICE__DISCORD__TIMEOUT_SECONDS` | Yes | Webhook request timeout |
| `SERVICE__DISCORD__MAX_RETRIES` | Yes | Webhook retry count |

**Outbound HTTP client** (`SERVICE__HTTP__…`)

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `SERVICE__HTTP__USER_AGENT` | Yes | — | User-Agent sent on all outbound requests |
| `SERVICE__HTTP__TIMEOUT_SECONDS` | Yes | — | Overall request timeout |
| `SERVICE__HTTP__CONNECT_TIMEOUT_SECONDS` | Yes | — | Connect-phase timeout |
| `SERVICE__HTTP__POOL_IDLE_TIMEOUT_SECONDS` | Yes | — | Idle connection lifetime |
| `SERVICE__HTTP__POOL_MAX_IDLE_PER_HOST` | Yes | — | Idle connections kept per host |
| `SERVICE__HTTP__ENABLE_GZIP` | No | `true` | Gzip compression for outbound requests |

**ETL pipeline** (`SERVICE__ETL__…`)

| Variable | Required | Description |
| --- | --- | --- |
| `SERVICE__ETL__FETCH_INTERVAL_HOURS` | Yes | Interval between scheduled fetch runs |
| `SERVICE__ETL__LOOKBACK_DAYS` | Yes | Days in the past to include per run |
| `SERVICE__ETL__LOOKAHEAD_DAYS` | Yes | Days in the future to include per run |
| `SERVICE__ETL__ALERT_COOLDOWN_HOURS` | Yes | Cooldown before re-alerting the same event |
| `SERVICE__ETL__BATCH_SIZE` | Yes | Records per DB persistence batch |
| `SERVICE__ETL__RETENTION__APPROACH_RETENTION_YEARS` | No | Default `5` — `prune` deletes `approaches` older than this |
| `SERVICE__ETL__RETENTION__ETL_EVENT_RETENTION_DAYS` | No | Default `30` — `prune` deletes `etl_events` older than this |
| `SERVICE__ETL__RETENTION__ETL_EVENTS_KEEP_MIN` | No | Default `50` — minimum `etl_events` rows kept regardless of age |
| `SERVICE__ETL__INTERNAL_EVENTS_URL` | No | `POST /internal/events` webhook URL the `load` command notifies after a run; unset skips publishing entirely (the database stays the source of truth either way) |

**HTTP server** (`SERVICE__SERVER__…`)

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `SERVICE__SERVER__REQUEST_TIMEOUT_SECONDS` | No | `300` | `TimeoutLayer` duration (→ 504) |
| `SERVICE__SERVER__RATE_LIMIT_REQUESTS` | No | `100` | `GovernorLayer` quota per period |
| `SERVICE__SERVER__RATE_LIMIT_PERIOD_SECONDS` | No | `60` | `GovernorLayer` quota window |
| `SERVICE__SERVER__MAX_HAZARD_SUBSCRIBERS` | No | `100` | Max concurrent `GET /api/events/hazards` SSE connections (→ 503 past cap) |
| `SERVICE__SERVER__INTERNAL_EVENT_RATE_LIMIT_REQUESTS` | No | `30` | Tighter per-minute `GovernorLayer` quota applied only to `POST /internal/events` |

**Internal event ingest** (read directly, not `SERVICE__`-prefixed — never a committed config file)

| Variable | Required | Description |
| --- | --- | --- |
| `INTERNAL_EVENT_TOKEN` | Yes (when `api` feature is on) | Shared secret compared in constant time against `X-Internal-Token` on `POST /internal/events`; the server **fails to start** if unset. The `load` command (and any other publisher) sends the same value. |

**Metrics** (`SERVICE__PROMETHEUS__…`, `SERVICE__GRAFANA_CLOUD_PROMETHEUS__…`) — both sections are fully optional; omit them entirely to run with local `/metrics` scraping only.

| Variable | Default | Description |
| --- | --- | --- |
| `SERVICE__PROMETHEUS__URL` | `""` | OTLP push endpoint (e.g. Grafana Cloud OTLP gateway) |
| `SERVICE__PROMETHEUS__QUERY_URL` | unset | Legacy Prometheus query API used as a fallback for the dashboard metrics widget |
| `SERVICE__PROMETHEUS__USERNAME` | `""` | Basic-auth user for OTLP push |
| `SERVICE__PROMETHEUS__TOKEN` | `""` | Basic-auth token for OTLP push |
| `SERVICE__PROMETHEUS__INTERVAL_SECONDS` | `60` | OTLP push interval |
| `SERVICE__GRAFANA_CLOUD_PROMETHEUS__URL` | `""` | Grafana Cloud Prometheus query API URL (preferred source for the dashboard widget) |
| `SERVICE__GRAFANA_CLOUD_PROMETHEUS__INSTANCE_ID` | `""` | Grafana Cloud instance ID (basic-auth user) |
| `SERVICE__GRAFANA_CLOUD_PROMETHEUS__TOKEN` | `""` | Grafana Cloud API token (basic-auth password) |

**JPL Sentry & SBDB clients** (`[jpl_sentry]` / `[jpl_sbdb]` in `config/config.toml`) — both sections are fully optional (`#[serde(default)]` on `JplSentryConfig`/`JplSbdbConfig`) and, unlike other config sections, are TOML-table-only — there's no `SERVICE__JPL_SENTRY__…` / `SERVICE__JPL_SBDB__…` env var override path.

| Field | Default | Description |
| --- | --- | --- |
| `jpl_sentry.base_url` | `https://ssd-api.jpl.nasa.gov/sentry.api` | JPL Sentry impact-monitoring API base URL |
| `jpl_sentry.request_delay_ms` | `1000` | Self-imposed courtesy delay between per-asteroid Sentry lookups (the API exposes no rate-limit headers) |
| `jpl_sentry.stale_days` | `30` | Days before `sentry` re-checks an already-checked asteroid |
| `jpl_sbdb.base_url` | `https://ssd-api.jpl.nasa.gov/sbdb.api` | JPL Small-Body Database API base URL |
| `jpl_sbdb.request_delay_ms` | `1000` | Self-imposed courtesy delay between per-asteroid SBDB lookups |
| `jpl_sbdb.stale_days` | `90` | Days before `orbits` re-fetches an asteroid's orbit (orbits change slowly, so this is longer than `jpl_sentry.stale_days`) |

> **Note:** `.env.example` also lists `SERVICE__SERVER__CACHE__ENABLED`. There is no `cache` field on `ServerConfig` and no response caching is currently implemented — it's a placeholder for the design captured in [`docs/CACHING_PLAN.md`](docs/CACHING_PLAN.md). Setting it currently has no effect.

Service identity fields (`name`, `env`, `host`, `port`, `log_level`) are also overridable via `SERVICE__SERVICE__NAME`, `SERVICE__SERVICE__ENV`, `SERVICE__SERVICE__HOST`, `SERVICE__SERVICE__PORT`, `SERVICE__SERVICE__LOG_LEVEL`, but in practice these are set once per environment in `config/development.toml` / `config/production.toml` rather than per-deploy.

## 🚀 Getting Started

### 📦 Prerequisites

| Tool | Purpose | Recommended Version |
| --- | --- | --- |
| `Rust` | Systems programming language | `1.95+` (edition 2024) |
| `Cargo` | Rust package manager | latest |
| `git` | Version control | latest |
| `Docker` | Containerization platform (optional) | latest |
| `PostgreSQL` | Relational database (for local dev) | `14+` |

### 🖥️ Local Development Setup

1. **Clone the repository**

   ```bash
   git clone https://gitlab.com/AhmadZaweet/rustroid-sentinel.git
   cd rustroid-sentinel
   ```

2. **Set up environment variables**

   ```bash
   cp .env.example .env
   ```

   Fill in at minimum `SERVICE__DATABASE__URL` and `SERVICE__NASA__API_KEY`. See [Environment Variables](#-environment-variables).

3. **Run the application**

   ```bash
   cargo run -- serve
   ```

   `api`, `alerting`, `metrics`, and `etl` are all default Cargo features, so a plain `cargo run` builds everything. Migrations run automatically on startup ([`src/database/mod.rs`](src/database/mod.rs) `run_migrations`). The server listens on `http://localhost:8000` by default (`config/config.toml`).

   To run migrations manually instead: `sqlx migrate run --database-url $SERVICE__DATABASE__URL` (requires `sqlx-cli`, in `[dev-dependencies]`).

4. **Build the frontend CSS** (required — the dashboard serves the compiled `static/css/dist.css`, not `input.css`, and it isn't checked in)

   ```bash
   npm install
   npm run build:css   # one-shot build via @tailwindcss/cli -> static/css/dist.css
   npm run watch:css    # rebuild on change, for template/CSS iteration
   ```

   `static/css/input.css` is the Tailwind + basecoat-ui source; `tailwind.config.js` holds the theme tokens (space/nebula/hazard color palette, font pairing). There is no CDN Tailwind fallback anymore — skipping this step means an unstyled dashboard.

### 🐳 Running with Docker

`docker-compose.yml` wires the app to a local Postgres 17 container with a healthcheck-gated startup:

```bash
docker compose up --build
```

Or standalone against an external database:

```bash
cp .env.example .env   # point SERVICE__DATABASE__URL at your database
docker build -t rustroid-sentinel:latest .
docker run -d -p 8000:8000 --env-file .env rustroid-sentinel:latest
```

The image entrypoint is `rustroid-sentinel serve`; override the CMD to run ETL subcommands instead (this is exactly what the scheduled GitLab CI `etl_pipeline` job does against the built image).

### 🧪 Running the Examples

```bash
cargo run --example api_client    # hits a running server's /api endpoints
cargo run --example basic_etl     # shells out through extract -> transform -> load
cargo run --example custom_alert  # sends one Discord embed using your configured webhook
```

## 🛠 CLI Commands

| Command | Feature required | Description |
| --- | --- | --- |
| `rustroid-sentinel extract [-s START] [-e END] [-o DIR] [-b BATCH_SIZE] [-f] [--dry-run]` | — | Fetches NEO data from NASA in date-range batches, writes raw JSON to `data/raw/` |
| `rustroid-sentinel transform [-i DIR] [-o DIR] [-f] [--dry-run]` | — | Converts raw JSON into domain models with hazard classification, writes NDJSON to `data/transformed/` |
| `rustroid-sentinel load [-i DIR] [-f] [--dry-run]` | — | Streams NDJSON into PostgreSQL via batched `UPSERT`s, tracked in `etl_events`; publishes newly-inserted Critical/High approaches to `internal_events_url` if configured (best-effort, never fails the run) |
| `rustroid-sentinel alert` | `alerting` | Finds unalerted hazardous approaches and sends Discord embeds |
| `rustroid-sentinel prune [--dry-run]` | — | Deletes `approaches`/`etl_events` rows past the configured retention window (`etl.retention.*`), keeping the database under its storage budget |
| `rustroid-sentinel sentry [--recompute]` | — | Checks `is_sentry_object` asteroids against JPL's Sentry API, stores real `torino_scale`/`palermo_scale`; `--recompute` ignores `sentry_checked_at` and re-checks everything |
| `rustroid-sentinel orbits [--recompute]` | — | Fetches orbital elements/spectral class/albedo from JPL's SBDB API into `asteroid_orbits`; `--recompute` ignores `orbit_checked_at` and re-fetches everything |
| `rustroid-sentinel vectorize` | — | Recomputes the 16-dim pgvector embedding for every asteroid and upserts it into `asteroid_embeddings` (pure computation, no staleness tracking — always full recompute) |
| `rustroid-sentinel report [--dry-run]` | `alerting` | Aggregates the trailing 7 days of approaches into a Discord summary embed |
| `rustroid-sentinel pipeline [--force-report] [--skip-report]` | `alerting` | Runs `extract → transform → load → alert → sentry → orbits → prune → vectorize → report` (report only on Sundays, unless `--force-report`; `--skip-report` omits it entirely) in one process, so one scheduled run wakes the database once instead of once per stage |
| `rustroid-sentinel serve` | `api` | Runs migrations, then starts the Axum HTTP server |

All eleven subcommands are defined in [`src/main.rs`](src/main.rs); `alert`, `report`, `pipeline` (feature `alerting`) and `serve` (feature `api`) are behind `#[cfg(feature = "...")]` and disappear from the binary if that feature is disabled at build time.

## 🔒 Rate Limiting & Security

| Mechanism | Where | Behavior |
| --- | --- | --- |
| Per-IP rate limit | `axum-governor` `GovernorLayer` ([`router.rs`](src/server/router.rs)) | `rate_limit_requests` per `rate_limit_period_seconds` per client IP (from `PeerIp` via `RealIpLayer`); `429` when exceeded |
| Internal webhook auth | `POST /internal/events` ([`internal_events.rs`](src/api/handlers/internal_events.rs)) | `X-Internal-Token` compared byte-for-byte in constant time against `INTERNAL_EVENT_TOKEN`; `401` on mismatch or missing header. Route is outside the `/api` router and not linked from the dashboard. Own tighter `GovernorLayer` bucket (`internal_event_rate_limit_requests`, default 30/min) and a 64 KB body cap layered inside the global 1 MiB limit |
| SSE subscriber cap | `GET /api/events/hazards` ([`hazard_events_sse.rs`](src/api/handlers/hazard_events_sse.rs)) | `503` once `max_hazard_subscribers` concurrent streams are held, since each is a live task |
| CSP + security headers | `axum-helmet` ([`router.rs`](src/server/router.rs)) | Explicit `script-src`/`style-src`/`font-src`/`img-src`/`connect-src` allowlist (self + named CDNs), not a wildcard |
| CORS | Custom `cors_middleware` ([`middleware.rs`](src/server/middleware.rs)) | Only `localhost:8000` / `127.0.0.1:8000` origins (or none) allowed; `GET` only; others get `403` |
| Request body size | `RequestBodyLimitLayer` ([`router.rs`](src/server/router.rs)) | Hard cap at 1 MiB |
| TLS verification | `reqwest` client config ([`api/client.rs`](src/api/client.rs)) | `danger_accept_invalid_certs(false)`, `danger_accept_invalid_hostnames(false)` on all outbound calls |
| Redirect limit | `reqwest` client config ([`api/client.rs`](src/api/client.rs)) | Max 5 redirects followed |
| Secret redaction | Manual `Debug` impls ([`settings.rs`](src/settings.rs)) | DB URL, NASA key, Discord webhook, Prometheus/Grafana tokens never appear in logs |
| Request timeout | `TimeoutLayer` ([`router.rs`](src/server/router.rs)) | `request_timeout_seconds` → `504 Gateway Timeout` |
| Dependency policy | [`deny.toml`](deny.toml) | Denies GPL/AGPL licenses and known-insecure crates (e.g. `openssl`); run via `cargo deny check` — not currently a CI gate |

## ❤️ Health Checks

Two distinct health endpoints exist with different semantics:

| Endpoint | Handler | Status code behavior | Used by |
| --- | --- | --- | --- |
| `GET /health` | `health_check_handler` ([`server/middleware.rs`](src/server/middleware.rs)) | Runs `SELECT 1`; returns **503** if the database is unreachable, else **200** | Docker `HEALTHCHECK`, load balancers |
| `GET /api/health` | `health` ([`api/handlers/health.rs`](src/api/handlers/health.rs)) | Always **200**; `database_connected: bool` is reported in the JSON body instead | Dashboard health widget |

The Docker image's own `HEALTHCHECK` (in [`Dockerfile`](Dockerfile)) polls `/api/health` every 30s.

## 📊 Observability

- **Tracing**: `tracing` + `tracing-subscriber` with `EnvFilter`, level controlled by `SERVICE__SERVICE__LOG_LEVEL` / `config/*.toml`; `TraceLayer` on every HTTP request.
- **Prometheus**: `HTTP_REQUESTS_TOTAL`, `HTTP_REQUEST_DURATION` (histogram, ms-to-10s buckets), `DATABASE_QUERIES_TOTAL`, `DATABASE_QUERY_DURATION` — all registered in [`src/metrics/registry.rs`](src/metrics/registry.rs), exposed at `GET /metrics` in Prometheus text format.
- **OTLP push**: the same request/query events are also recorded as OpenTelemetry counters/histograms and pushed to Grafana Cloud every 10 seconds when `SERVICE__PROMETHEUS__URL` is set ([`src/metrics/otlp.rs`](src/metrics/otlp.rs)).
- **Dashboard metrics widget**: `GET /api/metrics/summary` and `GET /dashboard/metrics` resolve metrics through a fallback chain — Grafana Cloud Prometheus query API → legacy `SERVICE__PROMETHEUS__QUERY_URL` → local Prometheus registry, always merged with live database counts ([`src/metrics/mod.rs`](src/metrics/mod.rs) `get_metrics_summary`).

## 🔄 CI/CD Pipeline

Defined in [`.gitlab-ci.yml`](.gitlab-ci.yml) and [`.gitlab/ci/`](.gitlab/ci/):

| Stage | Job | Trigger | What runs |
| --- | --- | --- | --- |
| `quality` | `compilation-check` | Merge request | `cargo fetch && cargo check --workspace --all-targets --locked` |
| `quality` | `formatting-check` | Merge request | `cargo fmt --all -- --check` then `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| `build` | `docker_build` | Push to default branch | `docker buildx build` with registry-backed layer cache, pushes `:latest` and `:$CI_COMMIT_SHORT_SHA` |
| `run-etl-pipeline` | `etl_pipeline` | Scheduled pipeline | Runs `rustroid-sentinel pipeline --skip-report` (extract → transform → load → prune → vectorize) then `rustroid-sentinel alert`, inside the just-built image |
| `run-weekly-report` | `weekly_report` | Scheduled pipeline, `$PIPELINE_SCHEDULE_TYPE == "weekly-report"` | Runs `rustroid-sentinel report`, inside the just-built image |

Both scheduled jobs gate on `$CI_PIPELINE_SOURCE == "schedule"`; `weekly_report` additionally requires a `PIPELINE_SCHEDULE_TYPE=weekly-report` CI/CD variable so it only fires from its own GitLab Pipeline Schedule (e.g. weekly, Sundays) and not the ETL schedule. Both **GitLab Pipeline Schedules must be created in the GitLab UI** (Settings → CI/CD → Pipeline Schedules) — cron cadence and the `PIPELINE_SCHEDULE_TYPE` variable live there, not in this repo. `--skip-report` on the ETL job keeps `pipeline`'s own Sunday auto-report from double-sending now that the report has its own dedicated schedule.

`cargo test` is **not** currently run as a CI stage — tests are run locally / via `.pre-commit-config.yaml` hooks, which also run `cargo fmt`, `cargo clippy --all-features -- -D warnings`, and `cargo check --release` before each commit.

## 🧪 Testing

Test dependencies actually exercised in the test suite:

- **`wiremock`** — mocks the NASA NeoWs HTTP API for both unit tests ([`src/nasa/asteroid_neows/api.rs`](src/nasa/asteroid_neows/api.rs)) and the end-to-end pipeline test ([`tests/e2e/full_pipeline.rs`](tests/e2e/full_pipeline.rs)).
- **`bollard`** — spins up a disposable `postgres:17-alpine` Docker container per integration test run for isolated database testing ([`tests/common/database.rs`](tests/common/database.rs)); chosen specifically over `testcontainers` due to a transitive RUSTSEC advisory (see the comment in [`Cargo.toml`](Cargo.toml)).
- Standard `#[tokio::test]` / `#[test]` unit tests are colocated with the code they cover throughout `src/`.

`mockall`, `proptest`, `insta`, and `assert-json-diff` are declared in `[dev-dependencies]` but are not currently used by any test.

```bash
# Run all tests
cargo test

# Run only unit tests (in src/)
cargo test --lib

# Run tests with output
cargo test -- --nocapture

# Generate coverage report (requires cargo-llvm-cov)
cargo llvm-cov --html --open
```

## 🤝 Contributing

Contributions are welcome! Please follow these steps:

1. **Fork the repository.**
2. **Create a new feature branch:** `git checkout -b feature/your-feature-name`
3. **Make your changes.**
4. **Commit your changes:** `git commit -m 'Add some feature'`
5. **Push to the branch:** `git push origin feature/your-feature-name`
6. **Submit a Merge Request.**

Please ensure tests are updated, follow `rustfmt` style and verify code with `cargo clippy`, and document public APIs with `///` and `# Errors` comments.

## 📜 License

This project is licensed under the **GNU General Public License v3.0**. See the [LICENSE](LICENSE) file for the full license text.

## 🙌 Acknowledgments

- [Rust](https://www.rust-lang.org/) - A language empowering everyone to build reliable and efficient software.
- [Axum](https://github.com/tokio-rs/axum) - Ergonomic web framework built with Tokio.
- [SQLx](https://github.com/launchbadge/sqlx) - Async Rust SQL toolkit with compile-time checked queries.
- [OpenTelemetry](https://opentelemetry.io/) - Standardization for high-quality, ubiquitous, and portable telemetry.
- [Serenity](https://github.com/serenity-rs/serenity) - A Rust library for the Discord API.
- [NASA Open APIs](https://api.nasa.gov/) - NeoWs API powers the NEO data for this platform.
