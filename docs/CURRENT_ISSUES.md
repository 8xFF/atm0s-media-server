# Current Documentation Issues

This page records known documentation limits found during the Reviewer pass. It is intentionally conservative: source-backed fixes should replace these notes when owners verify behavior or rewrite the affected docs.

## Stale Legacy Docs

The top-level docs are the current entry points. Several nested mdBook pages remain legacy and can conflict with source:

- `docs/user-guide/configuration.md` still documents removed subcommands and old flags such as `webrtc`, `rtmp`, `sip`, `token-generate`, `--zone-id`, and `--node-index`.
- `docs/contributor-guide/getting-started.md` still uses `--node-id` and a `webrtc` subcommand.
- `docs/contributor-guide/architecture.md` refers to `packages/endpoint`, `async_std::task::spawn`, SIP, and RTMP as current architecture.
- `docs/contributor-guide/middlewares/*.md` and `docs/contributor-guide/servers/media-server.md` reference old source paths such as `packages/endpoint` and `servers/media-server`.
- Placeholder or sparse pages remain under `docs/user-guide/upgrade.md`, `docs/getting-started/installation/kubernetes.md`, `docs/contributor-guide/features/recording.md`, and SDK/sample compatibility tables.

## Runtime Flags Needing Source Work Or Owner Confirmation

- `--http-tls` is parsed in `bin/src/main.rs`, but it is not passed into the inspected HTTP server startup paths in `bin/src/http.rs`.
- `--rtpengine-cmd-addr` is parsed by `bin/src/server/gateway.rs`, but no runtime use was found in the inspected gateway flow.
- `--enable-private-ip` defaults to `true`; no current `--disable-private-ip` style flag was found during review.

## Feature And Operations Gaps

- Recording code exists in media, connector, and `packages/media_record`, but end-to-end storage, hook, upload, and conversion operations were not run locally.
- Multi-tenancy sync is implemented in gateway and connector, but the external service contract and deployment workflow were not verified.
- Monitoring readiness is unclear: `/api/metrics/counts` exists and the console exists, but broader dashboard/operations claims need owner confirmation.
- SIP and RTMP are not current in-repo server modes or transport crates. Any SIP workflow should be documented as external integration through RTPengine-style APIs only after owner confirmation.

## External Or Generated Content Risks

- External release URLs, SDK repositories, and sample application repositories were not checked against the internet in this pass.
- `book/**` is generated mdBook output and may be stale after docs edits. Build docs from `book.toml` instead of editing generated files.
- `packages/media_console_front/react-app/dist/**`, `node_modules/**`, and TypeScript build info are local/generated frontend artifacts.
- Untracked scripts such as `bin/ows_media*.sh` and `packages/media_record/bin/run_*.sh` contain environment-specific values and should not be copied into public docs without owner review.

## Validation Limits From Reviewer Pass

- mdBook was built to a temporary output directory, not `book/**`.
- Local Markdown link validation checked target file existence only. It did not validate anchors or external URLs.
- Heavy Rust checks such as full `cargo test --all-features --workspace` and full clippy were not completed in this pass.
- The mdBook build emitted a version warning: `mdbook-mermaid` was built against mdBook `0.4.36`, while local `mdbook` was `0.4.18`.
- Local `cargo deny --all-features --target aarch64-apple-darwin check bans licenses sources` failed before checks because this installed cargo-deny does not recognize the `X11-swapped` term in `deny.toml`.
