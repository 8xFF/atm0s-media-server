# Installation

To install, you can either: 
- Install from Docker

```bash
docker run --net=host 8xff/atm0s-media-server:latest
```

- Download prebuild

```bash
wget https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-aarch64-apple-darwin
```

- Build from source

```
cargo build --release --package atm0s-media-server
```

Depend on your need, we have some ways to deploy atm0s-media-server:

- [Single zone](./single-zone.md)
- [Multi zones](./multi-zones.md)
- [Kubernetes](./kubernetes.md)