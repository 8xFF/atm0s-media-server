# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.2](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-core-v0.1.0-alpha.1...atm0s-media-server-core-v0.1.0-alpha.2) - 2025-02-08

### Other

- release 0.2.0-alpha.2 (#504)

## [0.1.0-alpha.1](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-core-v0.1.0-alpha.1) - 2025-02-08

### Added

- multi tenancy  (#433)
- rtp transport with HTTP APIs (#424)
- pubsub datachannel (#398)
- graceful disconnect with webrtc (#385)
- embedded userdata to token (#379)
- convert record to separated media files and push to s3 (#351)
- media record  (#329)
- connector (#316)
- audio mixer (#306)
- api gateway and session token for securing cluster (#292)
- webrtc sdk (#283)
- bitrate allocator with both egress and ingress. (#268)
- bitrate control with Twcc and Remb (#265)
- channel pub-sub feature and tests. cluster integration test (#262)
- add cluster metadata publish and subscribe options: peer and track info (#260)

### Fixed

- wrong usage of smallmap cause server crash. switched to indexmap (#457)
- webrtc transport stuck on connect_error cause memory leak (#453)
- handle video orientation from webrtc-extension (#452)
- crash assert on destroy (#449)
- endpoint internal clean up crash (#447)
- server crash because wrong ordered of remote stream destroy messages (#380)
- server crash if two sessions leaved with same room peer (#376)
- build warnings and clippy warnings (#328)
- typos and clippy warns (#296)
- missing clear room_map in cluster cause room failed to restart (#267)

### Other

- update version for release-plz (#500)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- ename peer's userdata to extra_data for avoid miss-understand (#386)
- fix clippy actions workflow and add cargo-fmt action (#353)
- more clippy fixes (#349)
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
