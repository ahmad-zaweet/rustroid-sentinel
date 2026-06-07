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
- [Key Objectives](#-key-objectives)
- [Project Structure](#-project-structure)
- [Getting Started](#-getting-started)
  - [Prerequisites](#-prerequisites)
  - [Local Development Setup](#-local-development-setup)
  - [Running with Docker](#-running-with-docker)
- [Environment Variables](#-environment-variables)
- [Testing](#-testing)
- [Contributing](#-contributing)
- [License](#-license)
- [Acknowledgments](#-acknowledgments)

---

## 📖 Overview

Rustroid Sentinel is a high-performance backend system built to directly integrate with the NASA NeoWs API, continually extracting and analyzing data on potentially hazardous near-Earth objects (NEOs). Built entirely in Rust using the `tokio` ecosystem, the service delivers a lightning-fast, highly concurrent data pipeline designed for demanding operational environments.

This repository showcases production-grade engineering principles. The architecture emphasizes modularity via discrete feature flags (`api`, `alerting`, `metrics`, `etl`), separating operations into independent ETL processes, Axum API serving, and Discord webhook alerting loops. Data integrity is enforced via `sqlx` async migrations and PostgreSQL. It incorporates modern observability through OpenTelemetry (OTLP) tracing and Prometheus metrics, comprehensive testing (`assert-json-diff`, `wiremock`, `testcontainers`), rate limiting (`axum-governor`), HTML templates (`askama`), and strict security header middlewares (`axum-helmet`), demonstrating a high standard of scalability and operational maturity.

## 💬 Discord Community

Join the official [Rustroid Sentinel Discord Server](https://discord.gg/GHT55B3Mdp) to connect with fellow developers, discuss space data pipelines, and receive real-time hazard alerts directly

## 🎯 Key Objectives

- **Deliver a Robust API**: Serve near-Earth object telemetry via an asynchronous, HTTP/2-ready Axum server capable of handling high loads efficiently with dynamic route handling and full security middlewares.
- **Ensure Data Integrity**: Extract and normalize NASA JSON feeds natively in Rust, employing structural pattern matching constraints and persisting strongly typed records securely into PostgreSQL.
- **Implement Instant Alerts**: Evaluate real-time hazard thresholds (via `serenity`) and trigger automated, rich webhook embeds directly to Discord channels upon detection of critical approach instances.
- **Promote Observability**: Expose detailed distributed application telemetry via `tracing`, and standard Prometheus reporting bridged to Grafana Cloud OTLP for seamless remote observability.
- **Enforce Code Reliability**: Guarantee resilience and zero UB by utilizing safe Rust guarantees alongside multi-layer containerized E2E testing, API endpoint mocking, and robust `anyhow`/`thiserror` boundaries.

## 📂 Project Structure

```text
rustroid-sentinel/
├── src/
│   ├── lib.rs              # Public library interface and re-exports
│   ├── main.rs             # Application entry point mapping CLI commands
│   ├── api/                # Axum REST endpoints, handlers, and presentation templates
│   ├── alert/              # Hazard threshold logic and Discord webhook pushes
│   ├── cli/                # Terminal command execution nodes
│   ├── database/           # PostgreSQL connection pools, migrations, and aggregations
│   ├── metrics/            # OTLP pushes, Prometheus registries, and tracing middlewares
│   ├── models/             # Shared domain data definitions and strict system types
│   ├── nasa/               # Typed HTTP client wrapping the NASA NeoWs endpoints
│   ├── server/             # API web server state, graceful shutdowns, and global routing
│   ├── settings.rs         # Strongly-typed environment override mapping layers
│   ├── transform/          # Data calculation and payload cleansing constraints
│   └── error.rs            # Top-level global application fault mapping
├── static/                 # Static web presentation assets (CSS, JS, Favicon)
├── templates/              # Askama HTML rendering templates
├── tests/                  # Integration tests spanning wiremock clients and container DBs
├── docs/                   # Extended internal architecture and testing documentation
├── Cargo.toml              # Rust crate manifest evaluating feature flags mapping
└── .env.example            # Environment variables scaffolding
```

## 🚀 Getting Started

### 📦 Prerequisites

| Tool         | Purpose                              | Recommended Version |
| ------------ | ------------------------------------ | ------------------- |
| `Rust`       | Systems programming language         | `1.85+`             |
| `Cargo`      | Rust package manager                 | latest              |
| `git`        | Version control                      | latest              |
| `Docker`     | Containerization platform (optional) | latest              |
| `PostgreSQL` | Relational database (for local dev)  | `14+`               |

### 🖥️ Local Development Setup

1. **Clone the Repository**

   ```bash
   git clone https://github.com/your-org/rustroid-sentinel.git
   cd rustroid-sentinel
   ```

2. **Install Dependencies** (if using coverage tools)

   ```bash
   cargo install sqlx-cli --no-default-features --features postgres
   cargo install cargo-llvm-cov
   ```

3. **Set Up Environment Variables**

   ```bash
   cp .env.example .env
   ```

   See the [Environment Variables](#-environment-variables) section for details.

4. **Run Database Migrations**

   ```bash
   sqlx migrate run --database-url $SERVICE__DATABASE__URL
   ```

5. **Run the Application**
   ```bash
   cargo run --all-features -- serve
   ```
   The application will be available at `http://localhost:8000`.

### 🐳 Running with Docker

1. **Set Up Environment Variables**

   ```bash
   cp .env.example .env
   ```

   Update `.env` with your configuration, ensuring `SERVICE__DATABASE__URL` points to the Docker database service.

2. **Build and Run**

   ```bash
   docker build -t rustroid-sentinel:latest .
   docker run -d -p 8000:8000 --env-file .env rustroid-sentinel:latest
   ```

3. **Viewing Logs**

   ```bash
   docker logs -f <container-id>
   ```

4. **Stopping the Application**
   ```bash
   docker stop <container-id>
   ```

## ⚙️ Environment Variables

| Variable                                     | Description                          | Example                                |
| -------------------------------------------- | ------------------------------------ | -------------------------------------- |
| `SERVICE__DATABASE__URL`                     | PostgreSQL connection string         | `postgresql://USER:PASS@HOST/DB`       |
| `SERVICE__DATABASE__MAX_CONNECTIONS`         | Max DB pool connections              | `10`                                   |
| `SERVICE__HTTP__USER_AGENT`                  | Base user agent for requests         | `rustroid-sentinel`                    |
| `SERVICE__SERVER__REQUEST_TIMEOUT_SECONDS`   | API Request Timeout                  | `300`                                  |
| `SERVICE__SERVER__RATE_LIMIT_REQUESTS`       | Total allowed requests               | `100`                                  |
| `SERVICE__SERVER__RATE_LIMIT_PERIOD_SECONDS` | Rate window duration                 | `60`                                   |
| `SERVICE__NASA__API_KEY`                     | NASA NeoWs API authentication key    | `DEMO_KEY`                             |
| `SERVICE__NASA__BASE_URL`                    | NASA Open API Base URL               | `https://api.nasa.gov`                 |
| `SERVICE__DISCORD__WEBHOOK_URL`              | Discord webhook for threshold alerts | `https://discord.com/api/webhooks/...` |
| `SERVICE__DISCORD__TIMEOUT_SECONDS`          | Action alert timeout                 | `30`                                   |
| `SERVICE__PROMETHEUS__URL`                   | Target OTLP ingestion URL            | `https://.../otlp`                     |
| `SERVICE__PROMETHEUS__INTERVAL_SECONDS`      | Recurring metrics push frequency     | `60`                                   |

## ✔️ Available Scripts

| Command                                  | Description                                |
| ---------------------------------------- | ------------------------------------------ |
| `cargo run -- serve`                     | Start the HTTP REST API server             |
| `cargo run --features etl -- extract`    | Fetch NEO data from NASA                   |
| `cargo run --features etl -- transform`  | Cleanse and model raw NEO data             |
| `cargo run --features etl -- load`       | Upsert transformed data into PostgreSQL    |
| `cargo run --features alerting -- alert` | Check hazard thresholds and trigger alerts |
| `cargo test`                             | Run all unit and integration tests         |
| `cargo doc --no-deps`                    | Generate local API documentation           |

## 🧪 Testing

This project uses Cargo's built-in test runner for unit and integration tests (mocking data with `wiremock` and integrating strongly typed isolated databases with `testcontainers`). Test data is supplied via fixtures in `tests/fixtures/`.

```bash
# Run all tests
cargo test

# Run only unit tests
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
6. **Submit a Pull Request.**

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
