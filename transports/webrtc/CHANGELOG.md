# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-transport-webrtc-v0.1.0...atm0s-media-server-transport-webrtc-v0.2.0) - 2024-01-24

### Added
- sip transport and hooks ([#167](https://github.com/8xFF/atm0s-media-server/pull/167))
- F32p2 conversion to from f32 ([#152](https://github.com/8xFF/atm0s-media-server/pull/152))
- auto or manual peer info subscribe ([#135](https://github.com/8xFF/atm0s-media-server/pull/135))
- audio mix-minus and allow subscribe multi sources ([#126](https://github.com/8xFF/atm0s-media-server/pull/126))
- gateway and refactor media-server ([#106](https://github.com/8xFF/atm0s-media-server/pull/106))

### Fixed
- *(deps)* update rust crate local-ip-address to 0.5.7 ([#178](https://github.com/8xFF/atm0s-media-server/pull/178))
- doctests sdp patch to ices ([#181](https://github.com/8xFF/atm0s-media-server/pull/181))
- whip/whep sdp patch with client ices failed [#176](https://github.com/8xFF/atm0s-media-server/pull/176) ([#179](https://github.com/8xFF/atm0s-media-server/pull/179))
- wrong track_id convert from random webrtc Mid ([#140](https://github.com/8xFF/atm0s-media-server/pull/140))
- webrtc stream missing info if sdk stop then create new with same name ([#100](https://github.com/8xFF/atm0s-media-server/pull/100))
- unused warn and local cluster aggregate bitrate ([#99](https://github.com/8xFF/atm0s-media-server/pull/99))

### Other
- restructure cargo workspace deps and fix [#122](https://github.com/8xFF/atm0s-media-server/pull/122) ([#125](https://github.com/8xFF/atm0s-media-server/pull/125))
- Feat connector server ([#120](https://github.com/8xFF/atm0s-media-server/pull/120))
- Bump udp_sas_async from 0.1.0 to 0.2.0 ([#97](https://github.com/8xFF/atm0s-media-server/pull/97))

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-transport-webrtc-v0.1.0) - 2023-11-23

### Added
- whip/whep protocol, embed js samples ([#76](https://github.com/8xFF/atm0s-media-server/pull/76))

### Fixed
- update deps version for avoiding *, updated atm0s-sdn to 0.1.1 ([#87](https://github.com/8xFF/atm0s-media-server/pull/87))
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))

### Other
- rename package. added release-plz for auto manage version ([#70](https://github.com/8xFF/atm0s-media-server/pull/70))
- update sdn, str0m. implement remb. fixed single video slow bootstrap ([#68](https://github.com/8xFF/atm0s-media-server/pull/68))
- update few simple unit tests ([#60](https://github.com/8xFF/atm0s-media-server/pull/60))
- Bump criterion from 0.4.0 to 0.5.1 ([#28](https://github.com/8xFF/atm0s-media-server/pull/28))
- Bump lz4_flex from 0.9.5 to 0.11.1 ([#27](https://github.com/8xFF/atm0s-media-server/pull/27))
- Update Rust crate flat2 to 1.0.28 ([#22](https://github.com/8xFF/atm0s-media-server/pull/22))
- update with newest sdn ([#21](https://github.com/8xFF/atm0s-media-server/pull/21))
- 17 integrate with bluesea sdn v4 ([#18](https://github.com/8xFF/atm0s-media-server/pull/18))
- cargo fmt
- dynamic payload type from remote ([#16](https://github.com/8xFF/atm0s-media-server/pull/16))
- update udp_sas for fixing unstable ([#14](https://github.com/8xFF/atm0s-media-server/pull/14))
- break between media-server and transports ([#12](https://github.com/8xFF/atm0s-media-server/pull/12))
