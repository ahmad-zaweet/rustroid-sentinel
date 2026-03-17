FROM lukemathwalker/cargo-chef:0.1.77-rust-alpine3.23 AS chef

WORKDIR /app

# Install build dependencies (required for your crates)
RUN apk add --no-cache musl-dev openssl-dev clang lld

### Step 1 - Plan
FROM chef AS planner

# Copy manifests only (changes rarely)
COPY Cargo.toml Cargo.lock ./

# Create minimal src stub for cargo to understand package structure
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Generate the recipe (dependency graph)
RUN cargo chef prepare --recipe-path recipe.json

### Step 2 - Build
FROM chef AS builder

# Copy the recipe from planner
COPY --from=planner /app/recipe.json ./recipe.json

# Cook dependencies only (this layer is cached by buildx)
RUN cargo chef cook --release --recipe-path recipe.json --features "api,alerting,metrics,etl"

# Copy only necessary source files (not tests, examples, docs)
COPY src ./src
COPY Cargo.toml ./

# Build the application
RUN cargo build --release --features "api,alerting,metrics,etl"

# Strip binary for smaller size
RUN strip target/release/rustroid-sentinel


### Step 3 - Run
FROM alpine:3.23 AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache ca-certificates openssl wget

# Copy the binary
COPY --from=builder /app/target/release/rustroid-sentinel /usr/local/bin/rustroid-sentinel

# Copy configuration files
COPY config /app/config
COPY migrations /app/migrations
COPY templates /app/templates
COPY static /app/static

# Create non-root user
RUN adduser -D sentinel
USER sentinel

# Environment
ENV RUST_LOG=warn
ENV RUN_ENV=production

# Expose port
EXPOSE 8000

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8000/api/health || exit 1

# Entrypoint
ENTRYPOINT ["rustroid-sentinel"]
CMD ["serve"]
