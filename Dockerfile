# ---- build stage ----
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /app

# кэшируем зависимости отдельным слоем
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY . .
# touch, чтобы cargo не взял закэшированный dummy-main.rs
RUN touch src/main.rs && cargo build --release

# ---- runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libc-bin \
    && rm -rf /var/lib/apt/lists/*
# libc-bin даёт getent, который использует get_docker_gid()

WORKDIR /app
COPY --from=builder /app/target/release/viskoz-cli /usr/local/bin/viskoz-cli

ENTRYPOINT ["viskoz-cli"]