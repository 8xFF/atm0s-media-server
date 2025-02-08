# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/media-server-protocol-v0.1.0) - 2025-02-08

### Added

- switch to manual2 discovery (#486)
- add duration_ms to compose record result (#451)
- record compose (#448)
- multi tenancy  (#433)
- rtp transport with HTTP APIs (#424)
- event hook (#420)
- pubsub datachannel (#398)
- transport for SIP with rtpengine protocol  (#359)
- embedded userdata to token (#379)
- media record  (#329)
- connector (#316)
- console API (#311)
- audio mixer (#306)
- api gateway and session token for securing cluster (#292)
- webrtc sdk (#283)
- bitrate allocator with both egress and ingress. (#268)
- bitrate control with Twcc and Remb (#265)
- add cluster metadata publish and subscribe options: peer and track info (#260)
- connector support http export transport (#233)
- connector with persistent queue  (#161)
- F32p2 conversion to from f32 (#152)
- connector external event log - protobuf (#132)

### Fixed

- handle video orientation from webrtc-extension (#452)
- add missing pagination to connector log apis (#363)
- api missing data (#355)
- typos and clippy warns (#296)
- *(deps)* update rust crate serde to 1.0.200 (#269)
- try fixing protoc release (#155)

### Other

- update metadata for packages (#492)
- update deps (#422)
- config zone id node id media port, get console lists (#417)
- ename peer's userdata to extra_data for avoid miss-understand (#386)
- cargo update and some libs (#381)
- fix clippy actions workflow and add cargo-fmt action (#353)
- run cargo update (#309)
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
- release server 0.1.1 (#123)
