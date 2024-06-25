FROM rust:latest as builder
ARG TARGET

WORKDIR /app
COPY . .

RUN rustup target add $TARGET && \
    cargo build --release --locked --target $TARGET

FROM debian:buster-slim

ARG TARGET

WORKDIR /app
COPY --from=builder /app/target/${TARGET}/release/netchat-server /usr/local/bin/netchat-server
COPY --from=builder /app/config /app/config

ENTRYPOINT ["netchat-server"]
