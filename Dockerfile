# ---- build ----
FROM rust:slim-bookworm AS builder

WORKDIR /app

# The whole dependency set is pure-Rust (rustls, no OpenSSL), so the build
# image needs no extra native packages.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

# ---- runtime ----
FROM debian:bookworm-slim

RUN useradd --create-home --uid 10001 app \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/stockinvoice /usr/local/bin/stockinvoice

USER app

ENV PORT=8080 \
    MONGO_URI=mongodb://mongo:27017/?directConnection=true \
    MONGO_DB=stockinvoice \
    RABBITMQ_URL=amqp://guest:guest@rabbitmq:5672/%2f

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/login >/dev/null || exit 1

ENTRYPOINT ["stockinvoice"]
