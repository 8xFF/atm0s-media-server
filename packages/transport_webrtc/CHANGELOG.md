# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/transport-webrtc-v0.1.0) - 2025-02-08

### Added

- multi tenancy  (#433)
- rtp transport with HTTP APIs (#424)
- pubsub datachannel (#398)
- graceful disconnect with webrtc (#385)
- embedded userdata to token (#379)
- media record  (#329)
- connector (#316)
- audio mixer (#306)
- api gateway and session token for securing cluster (#292)
- webrtc sdk (#283)
- bitrate allocator with both egress and ingress. (#268)
- bitrate control with Twcc and Remb (#265)
- add cluster metadata publish and subscribe options: peer and track info (#260)

### Fixed

- wrong usage of smallmap cause server crash. switched to indexmap (#457)
- webrtc transport stuck on connect_error cause memory leak (#453)
- handle video orientation from webrtc-extension (#452)
- crash assert on destroy (#449)
- endpoint internal clean up crash (#447)
- failed to parse h264 packet without simulcast (#441)
- firefox webrtc don't work with channel id 1000, switch back to 0 (#402)
- unsuccessful bind addr cause crash media node (#369)
- build warnings and clippy warnings (#328)
- typos and clippy warns (#296)

### Other

- update metadata for packages (#492)
- ename peer's userdata to extra_data for avoid miss-understand (#386)
- switched to internal deps from crate.io (#367)
- fix clippy actions workflow and add cargo-fmt action (#353)
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
