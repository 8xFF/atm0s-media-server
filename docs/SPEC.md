# Runtime And API Spec

This is a compact source-backed runtime/API specification for the current repository. Generated OpenAPI specs are available from running HTTP servers at each `/spec` path.

## CLI Shape

```text
atm0s-media-server [global-options] <subcommand> [subcommand-options]
```

Global options include:

- `--http-port <u16>`
- `--http-tls <u16>`: parsed, but not passed into the inspected HTTP server startup paths.
- `--sdn-port <u16>`
- `--sdn-zone-id <u32>`
- `--sdn-zone-node-id <u8>`
- `--sdn-zone-node-id-from-ip-prefix <prefix>`
- `--node-ip <ip>`
- `--node-ip-alt <ip>`
- `--node-ip-alt-cloud <provider>`
- `--enable-private-ip`
- `--enable-loopback-ip`
- `--enable-interfaces <comma-separated names>`
- `--enable-ipv6`
- `--secret <string>`
- `--seeds <node-addr>`
- `--seeds-from-url <url>`
- `--workers <usize>`
- `--sentry-endpoint <url>`

Clap `env` support is enabled for CLI args. Prefer explicit flags in examples because exact environment variable naming should be checked with `--help` for the built binary.

## Subcommands

### `standalone`

Important options:

- `--console-port`, default `8080`
- `--gateway-port`, default `3000`
- `--geo-db`, default `./maxminddb-data/GeoLite2-City.mmdb`
- `--media-instance-count`, default `2`
- connector options: `--db-uri`, `--s3-uri`, `--hook-uri`, `--hook-workers`, `--hook-body-type`
- record options: `--record-cache`, `--record-mem-max-size`, `--record-upload-worker`

### `gateway`

Important options:

- `--lat`, `--lon`
- `--geo-db`
- `--max-cpu`, `--max-memory`, `--max-disk`
- `--rtpengine-cmd-addr`: parsed, but no runtime use was found in the inspected gateway flow.
- `--multi-tenancy-sync`
- `--multi-tenancy-sync-interval-ms`

### `media`

Important options:

- `--enable-token-api`
- `--ice-lite`
- `--webrtc-port-seed`
- `--rtpengine-listen-ip`
- `--ccu-per-core`
- `--record-cache`
- `--record-mem-max-size`
- `--record-upload-worker`
- `--disable-gateway-agent`
- `--disable-connector-agent`

### `connector`

Important options:

- `--db-uri`, default `sqlite://connector.db?mode=rwc`
- `--s3-uri`
- `--hook-uri`
- `--hook-workers`
- `--hook-body-type`, default `protobuf-json`
- `--destroy-room-after-ms`
- `--storage-tick-interval-ms`
- `--multi-tenancy-sync`
- `--multi-tenancy-sync-interval-ms`

### `console`

No subcommand-specific options were found.

### `cert`

Important option:

- `--domains <domain>` repeated as needed.

## HTTP Surfaces

### Gateway HTTP

Gateway exposes:

- Token APIs: `/token/*`, `/token/ui`, `/token/spec`
- Media APIs: `/webrtc/*`, `/whip/*`, `/whep/*`, `/rtpengine/*`
- API docs/specs for each media API at `/webrtc/ui`, `/webrtc/spec`, and equivalent paths.
- Samples: `/samples`
- Node APIs: `/api/node/*`
- Metrics APIs: `/api/metrics/*`

### Media HTTP

Media exposes the same media, node, and metrics APIs. It exposes `/token/*` only when `--enable-token-api` is set.

### Console HTTP

Console exposes:

- `/` console frontend
- `/ws` console WebSocket
- `/api/user/*`
- `/api/cluster/*`
- `/api/connector/*`
- `/api/node/*`
- `/api/metrics/*`

### Connector HTTP

Connector exposes:

- `/api/node/*`
- `/api/metrics/*`

### Record Convert Worker

`packages/media_record/bin/convert_record_worker.rs` exposes:

- `POST /api/convert/job`
- `/api/docs`
- `/api/spec`

## Token Request Shapes

Token endpoints return `{ "status": true, "data": { "token": "..." } }` on success.

`POST /token/whip`:

```json
{
  "room": "room1",
  "peer": "publisher1",
  "ttl": 3600,
  "record": false,
  "extra_data": null
}
```

`POST /token/whep`:

```json
{
  "room": "room1",
  "peer": "viewer1",
  "ttl": 3600,
  "extra_data": null
}
```

`POST /token/webrtc`:

```json
{
  "room": "room1",
  "peer": "peer1",
  "ttl": 3600,
  "record": false,
  "extra_data": null
}
```

`POST /token/rtpengine`:

```json
{
  "room": "room1",
  "peer": "peer1",
  "ttl": 3600,
  "record": false,
  "extra_data": null
}
```

All token and media APIs use bearer tokens in the `Authorization` header.
