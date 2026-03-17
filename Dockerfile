FROM rust:1.94.0-alpine3.23 AS builder

WORKDIR /app

# Copy manifests first for caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy source code
COPY . .

# Build the application
RUN touch src/main.rs
RUN cargo build --release --features "api,alerting,metrics,etl


FROM alpine:3.23

WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Copy the binary
COPY --from=builder /app/target/release/rustroid-sentinel /usr/local/bin/rustroid-sentinel

# Copy configuration files
COPY config /app/config
COPY migrations /app/migrations
COPY templates /app/templates
COPY static /app/static

# Expose the API port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8000/api/health || exit 1

# Create a non-root user
RUN adduser -D sentinel
USER sentinel

# Set environment variables
ENV RUST_LOG=warn
ENV RUN_ENV=production

# Default command (can be overridden by CI/CD or docker run)
ENTRYPOINT ["rustroid-sentinel"]
CMD ["serve"]
