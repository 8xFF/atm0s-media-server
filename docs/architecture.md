# Server architecture

This document explain architecture of media-server and also architecture of each protocols.

### Entire is decentralized pub-sub and key-value

The media-server is designed as a global cluster in mind, where each media track is a global pubsub channel. This make us easier in scaling and also increase stability with decentralized architecture. Now we have a streaming system without single point of failed.

All server modules are trying to design with SAN I/O, but some parts are success some parts are not, we are trying to refactor to make it more stable and performane.

More about pub-sub and how we create decentralized streaming server can found here: [Smart-Routing](https://github.com/8xFF/atm0s-sdn/blob/master/docs/smart_routing.md)

### Terms

We have terms

- Gateway
- Media Server
- SessionId
- Transport
- Endpoint
- ConnId

The relate between each terms is described as bellow:

- Gateway holding list of resources. We have 2 type of gateways, first is zone level gateway which hold all servers inside it's zone. Second is global gateway, which hold all zone gateways. Gateway act as router for routing each user request to best node, or destination node base on type of requests.
- Each Users when connected to a server will have unique SessionId across all nodes (currently only SIP using it)
- Each Users joined to a room will be an Endpoint
- The connection between User and Endpoint is managed by a Transport like WebRTC, Whip, Whep, Rtmp, or SIP. This connection is identify by ConnId
- Each Server manage mutiple Endpoints

### Endpoint

Endpoint will be identified by ConnId, and is binding to a pair (room_id, peer_id)

### Multi transport-protocols

Currently transport will have 2 type of streams

- RPC streams: for controling, event firing
- Media streams: for audio/video send/recv

Currently we implementing some transport protocols as below list:

- [WebRTC](./protocols/webrtc.md) first citicen
- [Whip/Whep](./protocols/whip-whep.md)
- [RTMP](./protocols/rtmp.md)
- [SIP](./protocols/sip.md)
- [Media Over Quic](./protocols/moq.md)