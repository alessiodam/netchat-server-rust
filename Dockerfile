FROM rust:latest as builder

ARG TARGET

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev gcc-aarch64-linux-gnu

COPY . .
RUN mv config.toml.example config.toml

RUN rustup target add $TARGET && \
    if [ "$TARGET" = "aarch64-unknown-linux-gnu" ]; then \
        export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc; \
        export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++; \
        export PKG_CONFIG_ALLOW_CROSS=1; \
        export PKG_CONFIG_PATH_aarch64_unknown_linux_gnu=/usr/aarch64-linux-gnu/lib/pkgconfig; \
        export PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu; \
    fi && \
    cargo build --release --locked --target $TARGET

FROM ubuntu:latest

ARG TARGET

WORKDIR /app

RUN apt-get update && apt-get upgrade -y && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/entrypoint.sh /app/entrypoint.sh
COPY --from=builder /app/target/${TARGET}/release/netchat-server /usr/local/bin/netchat-server
COPY --from=builder /app/config.toml /app/config.toml

ENTRYPOINT ["bash", "/app/entrypoint.sh"]
