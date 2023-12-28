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
 <a href="https://discord.gg/qXr5zxsJWp">
  <img src="https://img.shields.io/discord/1173844241542287482?logo=discord" alt="Discord">
 </a>
</p>

# Decentralized Ultra-Low Latency Streaming Server

A decentralized media server designed to handle media streaming at a global-scale, making it suitable for large-scale applications but with minimal cost. It is designed with [SAN-I/O](https://sans-io.readthedocs.io/) in mind.

[<img src="https://img.youtube.com/vi/QF8ZJq9xuSU/hqdefault.jpg"
/>](https://www.youtube.com/embed/QF8ZJq9xuSU)

(Above is demo video of version used by Bluesea Network)

## Features
  - 🚀 Powered by Rust with memory safety and performance.
  - High availability by being fully decentralized, with no central controller.
  - 🛰️ Multi-zone support, high scalability.
  - Support encodings: H264, Vp8, Vp9, H265 (Coming soon), AV1 (Coming soon)
  - Cross platform: Linux, MacOs, Windows.
  - Decentralized WebRTC SFU (Selective Forwarding Unit)
  - Modern, full-featured client SDKs
    - [x] [Vanilla Javascript](https://github.com/8xFF/atm0s-media-sdk-js)
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
    - [x] SFU Cascading (each streams is global PubSub channel, similar to [Cloudflare interconnected network](https://blog.cloudflare.com/announcing-cloudflare-calls/))
    - [ ] Recording
    - [x] RTMP
    - [x] SIP (WIP)
    - [x] WebRTC
    - [x] Whip/Whep

## Getting started
To get started, you can either:
- Start from docker

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

Or create with token-generate api

```
atm0s-media-server --http-port 3100 token-generate
```

After that access http://localhost:3100/ui/ to create token by your self, deault cluster token is `insecure`

### Start a webrtc node only

For simple testing, we can start single node which support Webrtc for testing with Whip and Whep

```
atm0s-media-server --http-port 3200 webrtc
```

After that we can access `http://localhost:3000/samples` to see all embeded samples

### Start entire cluster

With cluster mode, we need each module in seperated node, we can run in single machine or multi machines with public or private network

Inner-Gateway module will route user trafic to best media-server node
```bash
atm0s-media-server --node-id 10 --sdn-port 10010 --http-port 3000 gateway
```

After that, gateway will print-out gateway address like: `10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001`, this address is used as seed node for other node joining to cluster

WebRTC module will serve user with SDK or Whip, Whep client
```bash
atm0s-media-server --node-id 21 --http-port 3001 --seeds 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 webrtc
```

RTMP module will serve user with RTMP broadcaster like OBS or Streamyard
```bash
atm0s-media-server --node-id 30 --seeds 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 rtmp
```

SIP module will serve user with sip-endpoint for integrating with Telephone provider.
```bash
atm0s-media-server --node-id 40 --seeds 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 sip
```

Now you can access sample page in url: http://localhost:3000/samples/webrtc/ in there we have 2 page: Whip broadcast and Whep viewer.

Note that, inner-gateway will select node based on usage so it will route to same media-server instance util it reach high usage. For testing media-exchange between system you can star more than one Webrtc module as you want:

```
atm0s-media-server --node-id 22 --http-port 3002 --seeds 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 webrtc
atm0s-media-server --node-id 23 --http-port 3003 --seeds 10@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 webrtc
```

After that you can direct access to samples on each WebRTC node:

First media-server: http://localhost:3001/samples/
Second media-server: http://localhost:3002/samples/
Third media-server: http://localhost:3003/samples/

![Demo Screen](./docs/imgs/demo-screen.jpg)

Each node also expose a metric dashboard here:

- Gateway: http://localhost:3000/dashboard/
- Media1: http://localhost:3001/dashboard/
- Media2: http://localhost:3002/dashboard/
- Media3: http://localhost:3003/dashboard/

![Monitoring](./docs/imgs/demo-monitor.png)

### Start RTMP session

Instead of publish with Whip client, we can publish rtmp stream by using any RTMP Client like OBS to publish to bellow stream:

- Server: `rtmp://NODE_IP:1935/live`
- Stream Key: `above generated rtmp token`

Stream codec should be config with h264 no B-Frame with ultra-low latency option.

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
The project is continuously being improved and updated. We are always looking for ways to make it better, whether that's through optimizing performance, adding new features, or fixing bugs. We welcome contributions from the community and are always looking for new ideas and suggestions.

For more information, you can join our [Discord channel](https://discord.gg/tJ6dxBRk)


## Roadmap
The first version will be released together with [our SDN](https://github.com/8xFF/decentralized-sdn) at the end of 2023.
Details on our roadmap can be seen [TBA]().

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

We would like to thank all the contributors who have helped in making this project successful.
