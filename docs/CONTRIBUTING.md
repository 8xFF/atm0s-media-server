# Contributing

Thanks for contributing to atm0s-media-server. This repository is currently in a refactoring-era state, so keep changes source-backed and scoped.

## Start Here

- Read [Developer Guide](./DEVELOPER_GUIDE.md) for repository layout and commands.
- Read [Architecture](./ARCHITECTURE.md) before changing runtime boundaries.
- Read [Runtime/API Spec](./SPEC.md) before changing CLI or HTTP APIs.
- Use existing crate boundaries and local patterns before adding new abstractions.

## Local Checks

From the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

Frontend:

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

## Documentation Rules

- Edit `docs/**`, not generated `book/**`.
- Document current source behavior, not planned behavior.
- Mark unknown or unverified details as `Unknown` or `Needs verification`.
- Do not copy secrets or environment-specific values from untracked scripts.
- Keep old nested docs in mind, but prefer the current top-level docs for user-facing entry points.

## Pull Requests

Use the local templates:

- [Issue template](./issue-template.md)
- [Pull request template](./pull-request-template.md)

For changes that touch CLI flags, server modes, HTTP routes, protocol files, or generated protobuf output, call that out explicitly in the PR description.
