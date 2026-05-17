# Developer Guide

This guide is for contributors and coding agents working in this repository. The current source of truth is the Rust workspace, not generated docs or older nested pages.

## Repository Map

| Path | Role |
| --- | --- |
| `bin/` | Main binary crate: CLI, server modes, HTTP API composition, Quinn virtual networking, seed refresh, helper scripts. |
| `packages/media_core` | Core endpoint, room, transport abstractions, and cluster room logic. |
| `packages/media_runner` | Sans-io media runtime worker that connects core, transports, gateway, and connector services. |
| `packages/transport_webrtc` | WebRTC, WHIP, and WHEP transport implementations. |
| `packages/transport_rtpengine` | RTPengine-style RTP transport worker. |
| `packages/media_gateway` | Gateway store and agent services, routing metadata, service selection state. |
| `packages/media_connector` | Connector handler/agent services, SQL persistence, hooks, retry queue helper. |
| `packages/media_record` | Raw record storage, upload service, conversion CLI, conversion worker. |
| `packages/media_secure` | JWT/token traits and implementations for edge, gateway, and console flows. |
| `packages/multi_tenancy` | App storage and sync client. |
| `packages/protocol` | Cluster IDs, endpoint/media/token types, protobuf source and generated RPC services. |
| `packages/media_console_front` | React console packaging, build script, frontend dev proxy/embedding. |
| `docs/` | Canonical mdBook source and top-level project docs. |
| `book/` | Generated mdBook output. Do not edit manually. |

## Prerequisites

- Rust `1.84.0`, `rustfmt`, and `clippy`.
- Linux packages used in CI: `libsoxr-dev`, `libopus-dev`, `libssl-dev`.
- `protoc` for all-features builds.
- Node 20 and pnpm for the console frontend.
- `mdbook` and `mdbook-mermaid` for docs builds.

## Core Commands

Run from the repository root unless noted.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
cargo build --release --package atm0s-media-server
cargo deny --all-features --target <target> check bans licenses sources
typos
```

Frontend console:

```bash
cd packages/media_console_front/react-app
pnpm install
pnpm lint
pnpm build
```

Docs:

```bash
mdbook build
```

`mdbook build` writes `book/**`; do not manually edit generated output.

## Protocol Generation

Source protobuf files live under `packages/protocol/proto/**`.

Generated Rust files live under `packages/protocol/src/protobuf/**`. Generation is controlled by the `build-protobuf` feature in `packages/protocol/build.rs`.

`packages/protocol/proto/sync.sh` is local-machine-specific because it copies from an absolute path outside this repository. Do not document or run it as a general setup step without rewriting it.

## Frontend Build Behavior

The React app lives in `packages/media_console_front/react-app`.

Release Rust builds run `pnpm install` and `pnpm run build` from `packages/media_console_front/build.rs` unless `SKIP_BUILD_CONSOLE_FRONT` is set. Non-debug builds embed `react-app/dist`; debug builds use a dev/static path.

## Testing Notes

Inline Rust tests exist across utility, security, WebRTC transport, record, connector, gateway, protocol, and console storage code. No top-level source `tests/` or `examples/` directory was found in this pass.

CI runs:

- `cargo test --all-features --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo deny check bans licenses sources --all-features --target <target>`

## Coding Agent Notes

- Read `IMPROVE_DOC_TASKS.md` and `.improve_doc/*.md` before large doc changes.
- Do not edit `book/**`.
- Do not copy values from untracked environment-specific scripts such as `bin/ows_media*.sh` or `packages/media_record/bin/run_*.sh`.
- Prefer current source paths over stale nested docs.
- Avoid broad rewrites of unrelated nested pages unless the task asks for it.
- Preserve user or generated changes in a dirty worktree.
