FROM rust:1-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runner

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ytbuch /usr/local/bin/ytbuch

ENV YTBUCH_HOST=0.0.0.0 \
    YTBUCH_PORT=8092
EXPOSE 8092

CMD ["ytbuch"]
