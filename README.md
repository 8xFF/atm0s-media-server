<p align="center">
 <a href="https://github.com/8xFF/atm0s-media-server/actions">
  <img src="https://github.com/8xFF/atm0s-media-server/actions/workflows/rust.yml/badge.svg?branch=master">
 </a>
 <a href="https://codecov.io/gh/8xff/atm0s-media-server">
  <img src="https://codecov.io/gh/8xff/atm0s-media-server/branch/master/graph/badge.svg">
 </a>
 <a href="https://deps.rs/repo/github/8xff/atm0s-media-server">
  <img src="https://deps.rs/repo/github/8xff/atm0s-media-server/status.svg">
 </a>
 <a href="https://crates.io/crates/atm0s-media-server">
  <img src="https://img.shields.io/crates/v/atm0s-media-server.svg">
 </a>
 <a href="https://docs.rs/atm0s-media-server">
  <img src="https://docs.rs/atm0s-media-server/badge.svg">
 </a>
 <a href="https://github.com/8xFF/atm0s-media-server/blob/master/LICENSE">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT">
 </a>
 <a href="https://discord.gg/qXr5zxsJWp">
  <img src="https://img.shields.io/discord/1173844241542287482?logo=discord" alt="Discord">
 </a>
</p>

# Decentralized Ultra-Low Latency Streaming Server

A decentralized media server designed to handle media streaming on a global scale, making it suitable for large-scale applications but with minimal cost. It is developed by 8xFF, a group of independent developers who are passionate about building a new generation of media server and network infrastructure with decentralization in mind. While we have received support from various companies and individuals, we are not affiliated with any specific company. 8xFF is a community-driven project, and we welcome anyone interested in contributing to join us.

For a deep dive into the technical aspects of network architecture, please refer to our [Smart-Routing](https://github.com/8xFF/atm0s-sdn/blob/master/docs/smart_routing.md)

[<img src="https://img.youtube.com/vi/QF8ZJq9xuSU/hqdefault.jpg"
/>](https://www.youtube.com/embed/QF8ZJq9xuSU)

(Above is a demo video of the version used by Bluesea Network)

## Project Status: Alpha

This project is currently in the alpha stage of development. It is actively being developed and may undergo significant changes, including changes to the API. The primary focus at this stage is to make the project stable and reliable.

Our goals for this project can be summarized as follows:

- **Goal 1**: Cluster: Create a global decentralized media server cluster with multiple zones, support both SDKs for flexible and mainstream protocols (RTMP, SIP, SRT) for compatibility.
- **Goal 2**: Market: Develop a sharing marketplace for the media server, enabling resource sharing and monetization. This will help scale the media server cluster during peak times and reduce costs during off-peak times. In the feature marketplace fees will be used to fund the development of the project.
- **Goal 3**: P2P Network: Establish a network between users, where servers only act as fallbacks. This approach aims to significantly reduce infrastructure costs and scale to infinity.

Our primary focus is currently on Goal 1. For details on the current phase plan, please refer to our [Roadmap](https://github.com/8xFF/atm0s-media-server/issues/69).

Goals 2 and 3 are in the research phase. For more information, you can visit this [Repository](https://github.com/giangndm/decentralized-p2p-streaming).

## Features

- 🚀 Powered by Rust with memory safety and performance.
- High availability by being fully decentralized, with no central controller.
- 🛰️ Multi-zone support, high scalability.
- Support encodings: H264, Vp8, Vp9, H265 (Coming soon), AV1 (Coming soon)
- Cross-platform: Linux, macOS, Windows.
- Decentralized WebRTC SFU (Selective Forwarding Unit)
- Modern, full-featured client SDKs
  - [x] [Vanilla JavaScript](https://github.com/8xFF/atm0s-media-sdk-js)
  - [x] [Rust](WIP)
  - [x] [React](https://github.com/8xFF/atm0s-media-sdk-react)
  - [x] [React Native](WIP)
  - [ ] Flutter
  - [ ] iOS Native
  - [ ] Android Native
- Easy to deploy: single binary, Docker, or Kubernetes
- Advanced features including:
  - [x] Audio Mix-Minus (WIP)
  - [x] Simulcast/SVC
  - [x] SFU
  - [x] SFU Cascading (each stream is a global PubSub channel, similar to [Cloudflare interconnected network](https://blog.cloudflare.com/announcing-cloudflare-calls/))
  - [ ] Recording
  - [x] RTMP
  - [x] SIP (WIP)
  - [x] WebRTC
  - [x] Whip/Whep

## Resources

- [Summary](./docs/summary.md)
- [Getting Started](./docs/getting-started/README.md)
- [User Guide](./docs/user-guide/README.md)
- [Contributor Guide](./docs/contributor-guide/README.md)
- [RFCs](https://github.com/8xff/RFCs)
- [FAQ](./docs/getting-started/faq.md)

## Quick Start

- From Docker

```bash
docker run --net=host ghcr.io/8xff/atm0s-media-server:master:master --help
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

Run first WebRTC node:

```bash
atm0s-media-server --http-port 3001 --zone-index=101 webrtc
```

After node1 started it will print out the node address like `101@/ip4/192.168.1.10/udp/10101/ip4/192.168.1.10/tcp/10101`, you can use it as a seed node for other nodes.

Run second WebRTC node:

```bash
atm0s-media-server --http-port 3002 --zone-index=102 --seeds FIRST_NODE_ADDR webrtc
```

Now two nodes will form a cluster and can be used for media streaming.

First media-server: http://localhost:3001/samples/whip/whip.html

Second media-server: http://localhost:3002/samples/whep/whep.html

You can use [Pregenerated-Token](./docs/getting-started/quick-start/whip-whep.md) to publish and play stream.

![Demo Screen](./docs/imgs/demo-screen.jpg)

Each node also has embedded monitoring page at `http://localhost:3001/dashboard/` and `http://localhost:3002/dashboard/`

![Monitoring](./docs/imgs/demo-monitor.png)

## Contributing

The project is continuously being improved and updated. We are always looking for ways to make it better, whether that's through optimizing performance, adding new features, or fixing bugs. We welcome contributions from the community and are always looking for new ideas and suggestions. If you find it interesting or believe it could be helpful, we welcome your contributions to the codebase or consider starring the repository to show your support and motivate our team!

For more information, you can join our [Discord channel](https://discord.gg/qXr5zxsJWp)

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

We would like to thank all the contributors who have helped in making this project successful.
