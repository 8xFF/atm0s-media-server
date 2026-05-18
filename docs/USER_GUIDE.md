# User Guide

This guide documents the current source-backed runtime for atm0s-media-server. Older nested pages may still mention removed command names such as `webrtc`, `rtmp`, `sip`, `token-generate`, `--zone-id`, or `--node-index`; use this page as the current entry point.

## Runtime Model

atm0s-media-server runs as one binary with subcommands:

| Mode | Purpose | HTTP surface when configured |
| --- | --- | --- |
| `standalone` | Local all-in-one console, gateway, connector, and media stack. | Console on `--console-port`, gateway on `--gateway-port`. |
| `console` | Cluster UI/API and seed discovery. | Console UI, `/ws`, `/api/user/*`, `/api/cluster/*`, `/api/connector/*`, `/api/node/*`, `/api/metrics/*`. |
| `gateway` | Token generation, media API entry point, destination selection. | `/token/*`, `/webrtc/*`, `/whip/*`, `/whep/*`, `/rtpengine/*`, `/samples`, `/api/node/*`, `/api/metrics/*`. |
| `media` | Media workers and WebRTC/RTPengine transports. | Media APIs and optionally `/token/*` when `--enable-token-api` is set. |
| `connector` | Connector event persistence, hooks, record upload handling. | `/api/node/*`, `/api/metrics/*`. Connector logs are queried through console RPC. |
| `cert` | Development utility for self-signed cert/key files. | None. |

## Local Standalone

Standalone is the simplest first run.

```bash
./download-geodata.sh
cd bin
cargo run -- \
  --sdn-zone-node-id 1 \
  --workers 1 \
  standalone \
  --geo-db ../maxminddb-data/GeoLite2-City.mmdb \
  --max-cpu 100 \
  --max-memory 100 \
  --max-disk 100
```

Then open:

- Console: `http://localhost:8080`
- Gateway API docs: `http://localhost:3000/token/ui`, `http://localhost:3000/webrtc/ui`, `http://localhost:3000/whip/ui`, `http://localhost:3000/whep/ui`, `http://localhost:3000/rtpengine/ui`
- Gateway samples: `http://localhost:3000/samples/whip/` and `http://localhost:3000/samples/whep/`

Use the same `--secret` value for nodes that need to join the same cluster. The default is `insecure` and is suitable only for local development.

## Separate Local Nodes

The tracked scripts in `bin/` show the current multi-process pattern:

```bash
cd bin
./z0_console_n0.sh
./z0_gate_n1.sh
./z0_media_n2.sh
./z0_media_n3.sh
./z0_connector_n4.sh
```

Those scripts use:

- `--sdn-zone-id` and `--sdn-zone-node-id` for node identity.
- `--seeds-from-url` to discover neighbors through console or gateway node APIs.
- `--enable-private-ip` for local/private network addresses.

For a manual seed, pass one or more `--seeds <node-addr>` values. A running node exposes its address at `/api/node/address`.

## Network And NAT Options

Global options apply before the subcommand:

- `--http-port <port>`: start the mode's HTTP server when the mode supports HTTP.
- `--sdn-port <port>`: bind SDN UDP on a fixed port. Default `0` asks the OS for a free port.
- `--node-ip <ip>`: manually select the bind IP and disable autodetection.
- `--node-ip-alt <ip>`: advertise additional IPs, useful behind NAT.
- `--node-ip-alt-cloud <provider>`: attempt public IP autodetection from a cloud metadata provider. Needs verification per provider/environment.
- `--enable-private-ip`, `--enable-loopback-ip`, `--enable-ipv6`, `--enable-interfaces <names>`: control autodetected bind addresses. Private IPv4 addresses are enabled by default in the current CLI.
- `--seeds-from-url <url>`: poll seed addresses from a node or console API.

## Tokens And Authentication

Media and token APIs use bearer tokens in the `Authorization` header.

Gateway exposes token generation endpoints:

- `POST /token/whip`
- `POST /token/whep`
- `POST /token/webrtc`
- `POST /token/rtpengine`

The token endpoints validate an app token through the gateway security layer. In single-tenant mode, the cluster `--secret` is used as the app secret. With `--multi-tenancy-sync`, app data is synced from the configured endpoint.

Media endpoints decode protocol-specific tokens:

- WHIP: room, peer, record flag, extra data.
- WHEP: room, optional peer, extra data.
- WebRTC SDK: optional room/peer, record flag, extra data.
- RTPengine: room, peer, record flag, extra data.

## Recording

Recording code exists in the media and record crates:

- Media nodes collect record packets and send upload requests through connector services.
- `packages/media_record` provides `convert_record_cli` and `convert_record_worker`.
- Connector storage and S3 URI configuration are used for record-related events and uploads.

End-to-end recording operations were not verified in this documentation pass, so treat recording as partial until your deployment validates storage, hooks, and conversion jobs.

## Unsupported In Current Source

No current RTMP or SIP server mode or transport crate was found. RTPengine APIs can be used as an integration boundary for RTP-style workflows, but the binary does not implement SIP signaling itself.
