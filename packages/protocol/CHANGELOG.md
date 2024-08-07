# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/media-server-protocol-v0.1.0) - 2024-06-25

### Added
- connector ([#316](https://github.com/8xFF/atm0s-media-server/pull/316))
- console API ([#311](https://github.com/8xFF/atm0s-media-server/pull/311))
- audio mixer ([#306](https://github.com/8xFF/atm0s-media-server/pull/306))
- api gateway and session token for securing cluster ([#292](https://github.com/8xFF/atm0s-media-server/pull/292))
- webrtc sdk ([#283](https://github.com/8xFF/atm0s-media-server/pull/283))
- bitrate allocator with both egress and ingress. ([#268](https://github.com/8xFF/atm0s-media-server/pull/268))
- bitrate control with Twcc and Remb ([#265](https://github.com/8xFF/atm0s-media-server/pull/265))
- add cluster metadata publish and subscribe options: peer and track info ([#260](https://github.com/8xFF/atm0s-media-server/pull/260))
- connector support http export transport ([#233](https://github.com/8xFF/atm0s-media-server/pull/233))
- connector with persistent queue  ([#161](https://github.com/8xFF/atm0s-media-server/pull/161))
- F32p2 conversion to from f32 ([#152](https://github.com/8xFF/atm0s-media-server/pull/152))
- connector external event log - protobuf ([#132](https://github.com/8xFF/atm0s-media-server/pull/132))

### Fixed
- typos and clippy warns ([#296](https://github.com/8xFF/atm0s-media-server/pull/296))
- *(deps)* update rust crate serde to 1.0.200 ([#269](https://github.com/8xFF/atm0s-media-server/pull/269))
- try fixing protoc release ([#155](https://github.com/8xFF/atm0s-media-server/pull/155))

### Other
- run cargo update ([#309](https://github.com/8xFF/atm0s-media-server/pull/309))
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
- release server 0.1.1 ([#123](https://github.com/8xFF/atm0s-media-server/pull/123))
