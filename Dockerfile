FROM rust:1.78-slim as builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin trendforge

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/trendforge .
COPY keys/ keys/
EXPOSE 8402
ENV RUST_LOG=info
CMD ["./trendforge", "--serve", "--port", "8402"]
