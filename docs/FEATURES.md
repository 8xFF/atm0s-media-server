# Features

This page describes features found in the current repository source.

| Feature | Current status | Notes |
| --- | --- | --- |
| Decentralized cluster | Present | Nodes use `atm0s-sdn`, zones, node IDs, seeds, and service discovery. |
| Standalone local mode | Present | Starts console, gateway, connector, and media nodes in one process. |
| Console UI/API | Present | Served by `console`; React frontend lives in `packages/media_console_front`. |
| WebRTC SDK API | Present | Protobuf-based HTTP API under `/webrtc/*`. |
| WHIP | Present | HTTP API under `/whip/*`; sample assets under `bin/public/media/whip`. |
| WHEP | Present | HTTP API under `/whep/*`; sample assets under `bin/public/media/whep`. |
| RTPengine-style API | Present | HTTP API under `/rtpengine/*` and transport crate `packages/transport_rtpengine`. |
| Token API | Present | `/token/whip`, `/token/whep`, `/token/webrtc`, `/token/rtpengine`. |
| Connector event storage | Present | SQL storage in `packages/media_connector`. |
| Connector hooks | Present | Optional `--hook-uri`, worker count, and hook body type. |
| Multi-tenancy sync | Present in gateway/connector | Uses `--multi-tenancy-sync` and sync interval flags. Exact external service contract needs verification. |
| Recording | Partial / needs verification | Media and record crates exist, including conversion CLI/worker. End-to-end operations were not verified. |
| Metrics counts | Present | `/api/metrics/counts`. Broader monitoring/dashboard readiness needs verification. |
| SIP | Not present in current binary | No SIP server mode or transport crate found. |
| RTMP | Not present in current binary | No RTMP server mode or transport crate found. |
| Media-over-QUIC | Not present | No current runtime implementation found in this pass. |

## Media API Endpoints

Gateway and media HTTP servers expose:

- `POST /webrtc/connect`
- `POST /webrtc/:conn_id/ice-candidate`
- `POST /webrtc/:conn_id/restart-ice`
- `POST /whip/endpoint`
- `PATCH /whip/conn/:conn_id`
- `DELETE /whip/conn/:conn_id`
- `POST /whep/endpoint`
- `PATCH /whep/conn/:conn_id`
- `DELETE /whep/conn/:conn_id`
- `POST /rtpengine/offer`
- `POST /rtpengine/answer`
- `PATCH /rtpengine/conn/:conn_id`
- `DELETE /rtpengine/conn/:conn_id`

## Admin And Runtime Endpoints

- Node address: `GET /api/node/address`
- Router dump: `GET /api/node/router_dump`
- Metrics counts: `GET /api/metrics/counts`
- Console login: `POST /api/user/login`
- Console cluster views: `GET /api/cluster/seeds`, `/consoles`, `/zones`, `/zones/:zone_id`
- Console connector logs: `GET /api/connector/:node/log/rooms`, `/peers`, `/sessions`, `/events`

## Codec Notes

The existing README previously listed several codec claims. This pass verified source presence for media codec crates and WebRTC handling, but did not verify a complete user-facing codec support matrix. Treat detailed codec support beyond current API behavior as Needs verification.
