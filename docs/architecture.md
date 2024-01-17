# Server architecture

This document explains the architecture of the media-server and also the architecture of each protocol.

## Entirely decentralized pub-sub and key-value

The media-server is designed as a global cluster, where each media track is a global pubsub channel. This decentralized architecture allows for easier scalability and increased stability. With this design, we have a streaming system that doesn't rely on a single point of failure.

All server modules are designed with SAN I/O in mind, but some parts are more successful than others. We are continuously working on refactoring to improve stability and performance.

For more information about pub-sub and how we create a decentralized streaming server, please refer to the [Smart-Routing](https://github.com/8xFF/atm0s-sdn/blob/master/docs/smart_routing.md) documentation.

## Terms

We use the following terms in our architecture:

- Gateway: Holds a list of resources. There are two types of gateways: zone level gateways, which hold all servers within their zone, and global gateways, which hold all zone gateways. Gateways act as routers, directing user requests to the best or destination node based on the request type and params.
- Media Server: The server responsible for handling media tracks and streams.
- SessionId: A unique identifier assigned to each user across all nodes. Currently, only SIP uses SessionId.
- Transport: Manages the connection between a user and an Endpoint using protocols such as WebRTC, Whip, Whep, Rtmp, or SIP. Each connection is identified by a ConnId.
- Endpoint: Represents a user joined to a room. Each Endpoint is identified by a ConnId and is bound to a pair of (room_id, peer_id).
- ConnId: The identifier for a connection between a user and an Endpoint.
- Server: Manages multiple Endpoints.

## Multi transport-protocols

The transport layer supports two types of streams:

- RPC streams: Used for controlling and event firing.
- Media streams: Used for audio/video send/receive.

Currently, we are implementing the following transport protocols:

- [WebRTC](./protocols/webrtc.md)
- [Whip/Whep](./protocols/whip-whep.md)
- [RTMP](./protocols/rtmp.md)
- [SIP](./protocols/sip.md)
- [Media Over Quic](./protocols/moq.md)