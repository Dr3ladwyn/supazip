# Build stage
FROM rust:1.92-bookworm AS builder
WORKDIR /app
COPY supazip/ .
RUN cargo build --release --locked -p supazip-cli && \
    strip target/release/supazip-cli

# Runtime stage — distroless
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/supazip-cli /usr/local/bin/supazip
ENTRYPOINT ["supazip"]
