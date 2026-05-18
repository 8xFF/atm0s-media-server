# atm0s-media-server

atm0s-media-server is a Rust workspace for a decentralized low-latency media server. The current runtime is a single binary with multiple server modes that can run together in `standalone` mode or as separate console, gateway, connector, and media nodes.

The project is in a refactoring-era state around `sans-io-runtime`. Trust the source tree over older nested docs where they disagree.

## Current Runtime

The binary is `atm0s-media-server` from `bin/`.

Current subcommands:

- `standalone`: starts in-process console, gateway, connector, and media nodes for local development.
- `console`: serves the console UI/API and SDN cluster views.
- `gateway`: serves token APIs, WebRTC/WHIP/WHEP/RTPengine media APIs, node APIs, metrics APIs, and sample assets.
- `media`: runs media workers and WebRTC/RTPengine transports.
- `connector`: stores connector events in SQL storage and handles record/hook related connector work.
- `cert`: writes self-signed certificate/key files for QUIC-related development utilities.

Implemented source-backed integrations include WebRTC SDK protobuf APIs, WHIP, WHEP, RTPengine-style APIs, recording upload/convert components, connector hooks, console APIs, and multi-tenancy sync storage. RTMP and SIP are not current in-repo server modes or transport crates.

## Quick Start From Source

Prerequisites:

- Rust `1.84.0` with `rustfmt` and `clippy` from `rust-toolchain.toml`.
- Linux packages used by CI: `libsoxr-dev`, `libopus-dev`, and `libssl-dev`.
- `protoc` for all-features builds.
- `wget` if using `download-geodata.sh`.
- Node 20 and pnpm if building the console frontend directly.

Download the GeoIP database used by gateway routing:

```bash
./download-geodata.sh
```

Run a local standalone stack from the `bin/` directory. Running from `bin/` also matches the debug sample asset path used by the HTTP server:

```bash
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

Default local ports in standalone mode:

- Console UI/API: `http://localhost:8080`
- Gateway media/token APIs: `http://localhost:3000`

Useful API docs are served by running nodes:

- Gateway token docs: `http://localhost:3000/token/ui`
- Gateway WHIP docs: `http://localhost:3000/whip/ui`
- Gateway WHEP docs: `http://localhost:3000/whep/ui`
- Gateway WebRTC docs: `http://localhost:3000/webrtc/ui`
- Gateway RTPengine docs: `http://localhost:3000/rtpengine/ui`
- Node APIs: `http://localhost:3000/api/node/ui`
- Metrics APIs: `http://localhost:3000/api/metrics/ui`

## Build And Check

From the repository root:

```bash
cargo build --release --package atm0s-media-server
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

Frontend console commands live in `packages/media_console_front/react-app`:

```bash
pnpm install
pnpm lint
pnpm build
```

## Documentation

Start with these current top-level docs:

- [User Guide](./docs/USER_GUIDE.md)
- [Developer Guide](./docs/DEVELOPER_GUIDE.md)
- [Architecture](./docs/ARCHITECTURE.md)
- [Features](./docs/FEATURES.md)
- [Runtime/API Spec](./docs/SPEC.md)
- [Troubleshooting](./docs/TROUBLESHOOTING.md)
- [Contributing](./docs/CONTRIBUTING.md)
- [Current Issues](./docs/CURRENT_ISSUES.md)

The mdBook source is `docs/**` and the generated output is `book/**`. Do not edit `book/**` manually.

## Known Caveats

- `--http-tls` is parsed by the CLI, but it is not passed into the inspected HTTP server startup paths.
- `--rtpengine-cmd-addr` is parsed by gateway args, but no runtime use was found in the inspected gateway flow.
- External release download URLs and external SDK/sample repositories were not verified in this documentation pass.
- Some older nested docs still describe removed or stale concepts. The top-level docs above are the current entry points.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) and [docs/CONTRIBUTING.md](./docs/CONTRIBUTING.md).

## License

This project is licensed under the MIT License. See [LICENSE](./LICENSE).
