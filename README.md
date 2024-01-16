<p align="center">
 <a href="https://github.com/8xFF/atm0s-media-server/actions">
  <img src="https://github.com/8xFF/atm0s-media-server/actions/workflows/rust.yml/badge.svg?branch=master">
 </a>
 <a href="https://codecov.io/gh/8xff/atm0s-media-server">
  <img src="https://codecov.io/gh/8xff/atm0s-media-server/branch/master/graph/badge.svg">
 </a>
<!--  <a href="https://deps.rs/repo/github/8xff/atm0s-media-server">
  <img src="https://deps.rs/repo/github/8xff/atm0s-media-server/status.svg">
 </a> -->
<!--  <a href="https://crates.io/crates/atm0s-media-server">
  <img src="https://img.shields.io/crates/v/8xff-sdn.svg">
 </a> -->
<!--  <a href="https://docs.rs/atm0s-media-server">
  <img src="https://docs.rs/8xff-sdn/badge.svg">
 </a> -->
 <a href="https://github.com/8xFF/atm0s-media-server/blob/master/LICENSE">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT">
 </a>
 <a href="https://discord.gg/qXr5zxsJWp">
  <img src="https://img.shields.io/discord/1173844241542287482?logo=discord" alt="Discord">
 </a>
</p>

# Decentralized Ultra-Low Latency Streaming Server

A decentralized media server designed to handle media streaming on a global scale, making it suitable for large-scale applications but with minimal cost. It is designed with [SAN-I/O](https://sans-io.readthedocs.io/) in mind.

For a deep dive into the technical aspects of network architecture, please refer to our [Smart-Routing](https://github.com/8xFF/atm0s-sdn/blob/master/docs/smart_routing.md)

[<img src="https://img.youtube.com/vi/QF8ZJq9xuSU/hqdefault.jpg"
/>](https://www.youtube.com/embed/QF8ZJq9xuSU)

(Above is a demo video of the version used by Bluesea Network)

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

## Getting started
To get started, you can either:
- Start from Docker

```bash
docker run --net=host 8xff/atm0s-media-server:latest
```

- Download prebuild

```bash
wget https://github.com/8xFF/atm0s-media-server/releases/download/latest/atm0s-media-server-aarch64-apple-darwin
```

- Or build from source

```
cargo build --release --package atm0s-media-server
```

### Prepare access token

- Pregenerated token for default config:

WHIP: `eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOiJwdWJsaXNoZXIiLCJwcm90b2NvbCI6IldoaXAiLCJwdWJsaXNoIjp0cnVlLCJzdWJzY3JpYmUiOmZhbHNlLCJ0cyI6MTcwMzc1MjI5NDEyMn0.EfRZK7eHMZ-TCG23-jst8TAKVfbiQhX21cxB2mSznAM`

WHEP: `eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOm51bGwsInByb3RvY29sIjoiV2hlcCIsInB1Ymxpc2giOmZhbHNlLCJzdWJzY3JpYmUiOnRydWUsInRzIjoxNzAzNzUyMzE1NTgyfQ.6XS0gyZWJ699BUN0rXtlLH-0SvgtMXJeXIDtJomxnig`

RTMP: `eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOiJydG1wIiwicHJvdG9jb2wiOiJSdG1wIiwicHVibGlzaCI6dHJ1ZSwic3Vic2NyaWJlIjpmYWxzZSwidHMiOjE3MDM3NTIzMzU2OTV9.Gj0uCxPwqsFfMFLX8Cufrsyhtb7vedNp3GeUtKQCk3s`

SDK: `eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOm51bGwsInByb3RvY29sIjoiV2VicnRjIiwicHVibGlzaCI6dHJ1ZSwic3Vic2NyaWJlIjp0cnVlLCJ0cyI6MTcwMzc1MjM1NTI2NH0.llwwbSwVTsyFgL_jYCdoPNVdOiC2jbtNb4uxxE-PU7A`

Or create with token-generated api

```
atm0s-media-server --http-port 3100 token-generate
```

After that, you can access http://localhost:3100/ui/ to create a token by yourself. The default cluster token is `insecure`

### Start a webrtc node only

For simple testing, we can start a single node which supports Webrtc for testing with Whip and Whep

```
atm0s-media-server --http-port 3200 webrtc
```

After that, we can access `http://localhost:3000/samples` to see all embedded samples

### Start entire cluster

In cluster mode, each module needs to be on a separate node. This setup can run on a single machine or multiple machines, whether they are connected via a public or private network.

The Inner-Gateway module routes user traffic to the most suitable media server node.
```bash
atm0s-media-server --node-id 10 --sdn-port 10010 --http-port 3000 gateway
```

Afterward, the gateway prints out its address in the format: 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001. This address serves as the seed node for other nodes joining the cluster.

The WebRTC module serves users with either an SDK or a Whip, Whep client.
```bash
atm0s-media-server --node-id 21 --http-port 3001 --seeds ABOVE_GATEWAY_ADDR webrtc
```

The RTMP module serves users with an RTMP broadcaster such as OBS or Streamyard.
```bash
atm0s-media-server --node-id 30 --seeds ABOVE_GATEWAY_ADDR rtmp
```

The SIP module serves users with a SIP endpoint for integration with telephone providers.
```bash
atm0s-media-server --node-id 40 --seeds ABOVE_GATEWAY_ADDR sip
```

You can now access the sample page at the URL: http://localhost:3000/samples/webrtc/. There, you will find two pages: Whip Broadcast and Whep Viewer.

Please note that the inner-gateway selects nodes based on usage, routing to the same media-server instance until it reaches high usage. For testing media exchange between systems, you can start more than one WebRTC module as needed.

```
atm0s-media-server --node-id 22 --http-port 3002 --seeds ABOVE_GATEWAY_ADDR webrtc
atm0s-media-server --node-id 23 --http-port 3003 --seeds ABOVE_GATEWAY_ADDR webrtc
```

Afterward, you can directly access the samples on each WebRTC node:

First media-server: http://localhost:3001/samples/
Second media-server: http://localhost:3002/samples/
Third media-server: http://localhost:3003/samples/

![Demo Screen](./docs/imgs/demo-screen.jpg)

Each node also exposes a metric dashboard here:

- Gateway: http://localhost:3000/dashboard/
- Media1: http://localhost:3001/dashboard/
- Media2: http://localhost:3002/dashboard/
- Media3: http://localhost:3003/dashboard/

![Monitoring](./docs/imgs/demo-monitor.png)

### Start RTMP session

Instead of publishing with the Whip client, we can use any RTMP client, such as OBS, to publish to the following stream:

- Server: `rtmp://RTMP_NODE_IP:1935/live`
- Stream Key: `above generated rtmp token`

The stream codec should be configured with h264, without B-frames, and with the ultra-low latency option, as shown in the screenshot below.

![Monitoring](./docs/imgs/demo-rtmp-config.png)

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

## Contributing
The project is continuously being improved and updated. We are always looking for ways to make it better, whether that's through optimizing performance, adding new features, or fixing bugs. We welcome contributions from the community and are always looking for new ideas and suggestions. If you find it interesting or believe it could be helpful, we welcome your contributions to the codebase or consider starring the repository to show your support and motivate our team!

For more information, you can join our [Discord channel](https://discord.gg/qXr5zxsJWp)

## Roadmap
The first version will be released together with [our Overlay Network](https://github.com/8xFF/atm0s-sdn).
Details on our roadmap can be seen [Roadmap](https://github.com/8xFF/atm0s-media-server/issues/69).

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

We would like to thank all the contributors who have helped in making this project successful.
