# Build stage
FROM rust:1.93-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
# Create dummy main.rs to cache dependency builds
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
# Now copy real source and build
RUN rm -rf src target/release/weather-bingo-api target/release/deps/weather_bingo_api*
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libpq5 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/weather-bingo-api .
COPY migrations ./migrations

ENV PORT=8080
EXPOSE 8080

CMD ["./weather-bingo-api"]
