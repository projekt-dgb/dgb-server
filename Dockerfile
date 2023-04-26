# -*- mode: dockerfile -*-
# syntax=docker/dockerfile:1.2

FROM rust:1.63.0-slim-bullseye as builder
WORKDIR /dgb-server
COPY . .
RUN apt-get update && apt install -y gcc make build-essential libsodium-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt install -y git && rm -rf /var/lib/apt/lists/*
COPY --from=builder /dgb-server/target/release/dgb-server /usr/local/bin/dgb-server
EXPOSE 8080
EXPOSE 443
EXPOSE 9418
CMD dgb-server start --ip 0.0.0.0
