# Troubleshooting

## GeoIP Database Missing

Gateway and standalone open the GeoIP database path configured by `--geo-db`. The default is `./maxminddb-data/GeoLite2-City.mmdb`.

Fix:

```bash
./download-geodata.sh
```

If running from `bin/`, pass:

```bash
--geo-db ../maxminddb-data/GeoLite2-City.mmdb
```

## Sample Pages 404 In Debug Builds

Debug HTTP servers serve samples from `./public/media/`. That path exists under `bin/public/media`, so run source-mode gateway or standalone from the `bin/` directory if you need local sample pages.

```bash
cd bin
cargo run -- --sdn-zone-node-id 1 standalone --geo-db ../maxminddb-data/GeoLite2-City.mmdb
```

Release builds embed the sample assets.

## Nodes Do Not Discover Each Other

Check:

- All nodes use the same `--secret`.
- Zone/node IDs are unique within the intended zone.
- `--sdn-port` is reachable over UDP.
- Seed URLs return node addresses, for example `http://localhost:3000/api/node/address`.
- `--enable-private-ip`, `--enable-loopback-ip`, or `--enable-interfaces` allow the address you expect.

Useful endpoints:

- `GET /api/node/address`
- `GET /api/node/router_dump`
- `GET /api/cluster/seeds?zone_id=0&node_type=Gateway` on console.

## Token Requests Return `APP_TOKEN_INVALID`

Token endpoints validate the bearer token as an app token. In single-tenant mode, use the same value as the cluster `--secret`. In multi-tenant mode, verify the app exists in the configured `--multi-tenancy-sync` source.

## Media API Returns 400 Or 403

Common causes:

- Missing or invalid bearer token.
- Token type does not match the endpoint, for example a WHIP token used on WHEP.
- WebRTC token room/peer does not match the protobuf join request.
- SDP body or content type does not match the endpoint.

Use the OpenAPI UI from the running node to inspect expected request shapes.

## Build Fails On Linux System Libraries

Install the packages used by CI:

```bash
sudo apt-get update
sudo apt install -y libsoxr-dev libopus-dev libssl-dev
```

## All-Features Build Fails Looking For Protobuf Tools

Install `protoc`. CI uses `arduino/setup-protoc` with version `25.1`.

## Console Frontend Build Fails

The frontend expects pnpm and Node 20-compatible tooling.

```bash
cd packages/media_console_front/react-app
pnpm install
pnpm build
```

Release Rust builds run the frontend build unless `SKIP_BUILD_CONSOLE_FRONT` is set.

## Recording Conversion Is Not Working

Recording is present in source but end-to-end operations need deployment verification. Check:

- Connector `--s3-uri`.
- Media `--record-cache`, `--record-mem-max-size`, and `--record-upload-worker`.
- Record worker authentication and `/api/convert/job` request body.
- Hook receiver availability if compose hooks are expected.

Do not copy values from untracked local record scripts; they may contain environment-specific credentials.

## TLS Flag Confusion

`--http-tls` is parsed by the CLI, but it is not passed into the inspected HTTP server startup paths. Treat HTTP TLS as unavailable until source owners wire and verify it.
