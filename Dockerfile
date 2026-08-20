# syntax=docker/dockerfile:1.7
FROM rust:1.93.1-bookworm AS builder
WORKDIR /source
COPY . .
RUN cargo build --locked --profile minimal-release -p underprint-server

FROM debian:bookworm-slim
LABEL org.opencontainers.image.source="https://github.com/Nano112/underprint"
LABEL org.opencontainers.image.licenses="MIT"
WORKDIR /app
COPY --from=builder /source/target/minimal-release/underprint-server /usr/local/bin/underprint-server
USER 65532:65532
ENV UNDERPRINT_BIND=0.0.0.0:8080 \
    UNDERPRINT_MODELS_DIR=/models \
    UNDERPRINT_MAX_CONCURRENCY=2 \
    UNDERPRINT_REQUESTS_PER_SECOND=10 \
    UNDERPRINT_REQUEST_TIMEOUT_SECONDS=30 \
    RUST_LOG=underprint_server=info,tower_http=info
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["underprint-server", "--healthcheck", "127.0.0.1:8080"]
ENTRYPOINT ["/usr/local/bin/underprint-server"]
