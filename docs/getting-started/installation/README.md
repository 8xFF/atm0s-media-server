# Installation

Atm0s-media-server is built into a single executable file, it can be get by some ways:

- Install from Docker

```bash
docker run --net=host ghcr.io/8xff/atm0s-media-server:master --help
```

- Download prebuild

| OS    | Arch         | Link                                                                                                                          |
| ----- | ------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| MacOS | aarch64      | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-aarch64-apple-darwin)       |
| MacOS | x86_64       | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-x86_64-apple-darwin)        |
| Linux | aarch64 gnu  | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-aarch64-unknown-linux-gnu)  |
| Linux | x86_64 gnu   | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-x86_64-unknown-linux-gnu)   |
| Linux | aarch64 musl | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-aarch64-unknown-linux-musl) |
| Linux | x86_64 musl  | [Download](https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-x86_64-unknown-linux-musl)  |

- Build from source

```
cargo build --release --package atm0s-media-server
./target/release/atm0s-media-server --help
```

Depend on your need, we have some topology to install atm0s-media-server:

- [Single zone](./single-zone.md)
- [Multi zones](./multi-zones.md)

Or you can use some tools to deploy atm0s-media-server:

- [Kubernetes](./kubernetes.md)
- [Docker Compose](./docker-compose.md)
