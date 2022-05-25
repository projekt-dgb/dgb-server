# syntax=docker/dockerfile:1.2

FROM ubuntu:latest
WORKDIR /dgb-server
COPY target/x86_64-unknown-linux-musl/release/dgb-server .
EXPOSE 8080
CMD ["./dgb-server", "start" "--ip", "0.0.0.0"]
