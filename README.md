# dgb-server

## Installation

```sh
sudo apt install musl-tools lld
rustup override set 1.60.0
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
docker build -t dgb-server .
docker save dgb-server | bzip2 | pv | ssh user@host docker load
```
