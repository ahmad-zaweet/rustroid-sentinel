# Contributing to Rustroid Sentinel

First off, thank you for considering contributing to Rustroid Sentinel. Your insights and contributions are valuable.

## Local Setup

1. **Fork & Clone** the repository.
2. **Install PostgreSQL** and create a database (`createdb rustroid_sentinel`).
3. **Copy `.env.example`** to `.env` and fill out your local DB URL and API keys.
4. **Run Migrations** (if any exist using `sqlx-cli`, or via code).
5. **Compile** using `cargo build`.

## Pull Request Process

1. **Branch Format**: `feature/add-my-feature` or `fix/issue-number`.
2. **Write Tests**: Ensure any logic added has corresponding unit tests or documentation tests.
3. **Run Checks**: Before submitting, ensure `cargo clippy` and `cargo fmt` pass natively.
4. **Review**: Wait for a maintainer to review and merge your PR.

## Code Style

- We strictly adhere to `rustfmt`. Do not commit manually formatted files.
- Ensure all public APIs are documented with `///` and `# Errors` / `# Panics` sections where appropriate.
