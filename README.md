<p align="center">
 <a href="https://github.com/8xFF/decentralized-media-server/actions">
  <img src="https://github.com/8xFF/decentralized-media-server/actions/workflows/rust.yml/badge.svg?branch=master">
 </a>
 <a href="https://codecov.io/gh/8xff/decentralized-media-server">
  <img src="https://codecov.io/gh/8xff/decentralized-media-server/branch/master/graph/badge.svg">
 </a>
 <a href="https://deps.rs/repo/github/8xff/decentralized-sdn">
  <img src="https://deps.rs/repo/github/8xff/decentralized-sdn/status.svg">
 </a>
<!--  <a href="https://crates.io/crates/8xff-media-server">
  <img src="https://img.shields.io/crates/v/8xff-sdn.svg">
 </a> -->
<!--  <a href="https://docs.rs/8xff-media-server">
  <img src="https://docs.rs/8xff-sdn/badge.svg">
 </a> -->
 <a href="https://github.com/8xFF/decentralized-media-server/blob/master/LICENSE">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT">
 </a>
 <a href="https://discord.gg/tJ6dxBRk">
  <img src="https://img.shields.io/discord/1173844241542287482?logo=discord" alt="Discord">
 </a>
</p>

# 8xFF Media Server: Distributed Ultra-Low Latency Streaming Server

A distributed media server designed to handle media streaming at a global-scale, making it suitable for large-scale applications but with minimal cost. It is designed with [SAN-I/O](https://sans-io.readthedocs.io/) in mind.

TODO: image about endpoints + connections

## Features
  - 🚀 Powered by Rust with memory safety and performance.
  - High availability by being fully distributed, with no central controller.
  - 🛰️ Multi-zone support, high scalability.
  - Support encodings: H264, Vp8, Vp9, H265 (Coming soon), AV1 (Coming soon)
  - Cross platform: Linux, MacOs, Windows.
  - Decentralized WebRTC SFU (Selective Forwarding Unit)
  - Modern, full-featured client SDKs
    - [x] [Vanilla Javascript]()
    - [x] [Rust]()
    - [x] [React]()
    - [x] [React Native]()
    - [ ] Flutter
    - [ ] iOS Native
    - [ ] Android Native
  - Easy to deploy: single binary, Docker, or Kubernetes
  - Advanced features including:
    - [ ] Audio Mix-Minus (WIP)
    - [x] Simulcast/SVC
    - [x] SFU
    - [x] SFU Cascading (each streams is global PubSub channel, similar to [Cloudflare interconnected network](https://blog.cloudflare.com/announcing-cloudflare-calls/))
    - [ ] Recording
    - [x] RTMP
    - [ ] SIP (WIP)
    - [x] WebRTC
    - [ ] Whip/Whep



## Live Demos

  - SDK demos: [TBA]()
  - Gather.io Clone: [TBA]()
  - Meeting Sample: [TBA]()
  - Broadcasting Sample: [TBA]()

## Docs

WIP

## Architecture

- Global Gateway
- Inner zone gateway
- Media Server
- Connector (connect to custom logic)

TODO: Diagram

## Getting started
To get started, you can either:
- Start from docker

```bash
docker run --net=host 8xff/media-server:latest
```

- Download prebuild

```bash
wget ....
```

- Or build from source

```
cargo build --package ...
```

### Start a single node

```
RUST_LOG=info media-server --enable-demos
```

After that we can access `http://localhost:3000/demos` to see all demos

### Start multi-nodes

```bash
RUST_LOG=info media-server --node-id 1 --sdn-port 5001 --enable-demos
```

```bash
RUST_LOG=info media-server --node-id 2 --sdn-port 5002 --neighbour-addr udp+p2p://NODE1_IP:5001 --enable-demos
```

After that we can access demo from both nodes:

```
http://NODE1_IP:3000/demos
or
http://NODE2_IP:3000/demos
```

Whenerever user access to demos on node1 or node2, users will see each other like single nodes

### Start RTMP nodes

We can enable rtmp by setting `--rtmp-port 1935` when starting a node, by that way we can publish rtmp stream by using any RTMP Client like OBS to publish to bellow stream:

- Server: `rtmp://NODE_IP:1935/live`
- Stream Key: `app?room=ROOM_ID&peer=PEER_ID`

Stream codec should be config with h264 no B-Frame with ultra-low latency option.

More info in [Publish Demo]()

### Start SIP gateway

TODO

## Contributing
The project is continuously being improved and updated. We are always looking for ways to make it better, whether that's through optimizing performance, adding new features, or fixing bugs. We welcome contributions from the community and are always looking for new ideas and suggestions.

For more information, you can join our [Discord channel](https://discord.gg/tJ6dxBRk)


## Roadmap
The first version will be released together with [our SDN](https://github.com/8xFF/decentralized-sdn) at the end of 2023.
Details on our roadmap can be seen [TBA]().

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

We would like to thank all the contributors who have helped in making this project successful.
