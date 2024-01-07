# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-v0.1.0...atm0s-media-server-v0.1.1) - 2024-01-07

### Added
- F32p2 conversion to from f32 ([#152](https://github.com/8xFF/atm0s-media-server/pull/152))
- node info endpoint ([#151](https://github.com/8xFF/atm0s-media-server/pull/151))
- connector external event log - protobuf ([#132](https://github.com/8xFF/atm0s-media-server/pull/132))
- implement secure with static key JWT, update atm0s-sdn, update readme ([#129](https://github.com/8xFF/atm0s-media-server/pull/129))
- audio mix-minus and allow subscribe multi sources ([#126](https://github.com/8xFF/atm0s-media-server/pull/126))
- gateway and refactor media-server ([#106](https://github.com/8xFF/atm0s-media-server/pull/106))
- auto or manual peer info subscribe ([#135](https://github.com/8xFF/atm0s-media-server/pull/135))

### Fixed
- missing dashboard in gateway and live sessions not update when session ended ([#111](https://github.com/8xFF/atm0s-media-server/pull/111))
- wrong typos cause publish error ([#93](https://github.com/8xFF/atm0s-media-server/pull/93))
- *(deps)* update rust crate quote to 1.0.35 ([#139](https://github.com/8xFF/atm0s-media-server/pull/139))
- *(deps)* update rust crate syn to 2.0.48 ([#128](https://github.com/8xFF/atm0s-media-server/pull/128))
- *(deps)* update rust crate syn to 2.0.42 ([#124](https://github.com/8xFF/atm0s-media-server/pull/124))
- *(deps)* update rust crate syn to 2.0.41 ([#110](https://github.com/8xFF/atm0s-media-server/pull/110))
- wrong track_id convert from random webrtc Mid ([#140](https://github.com/8xFF/atm0s-media-server/pull/140))
- webrtc stream missing info if sdk stop then create new with same name ([#100](https://github.com/8xFF/atm0s-media-server/pull/100))
- unused warn and local cluster aggregate bitrate ([#99](https://github.com/8xFF/atm0s-media-server/pull/99))

### Other
- *(deps)* bump rust-embed from 8.1.0 to 8.2.0 ([#142](https://github.com/8xFF/atm0s-media-server/pull/142))
- *(deps)* bump clap from 4.4.11 to 4.4.13 ([#149](https://github.com/8xFF/atm0s-media-server/pull/149))
- restructure cargo workspace deps and fix [#122](https://github.com/8xFF/atm0s-media-server/pull/122) ([#125](https://github.com/8xFF/atm0s-media-server/pull/125))
- Bump clap from 4.4.10 to 4.4.11 ([#104](https://github.com/8xFF/atm0s-media-server/pull/104))
- Bump rust-embed from 8.0.0 to 8.1.0 ([#109](https://github.com/8xFF/atm0s-media-server/pull/109))
- Feat connector server ([#120](https://github.com/8xFF/atm0s-media-server/pull/120))
- release ([#88](https://github.com/8xFF/atm0s-media-server/pull/88))
- Bump udp_sas_async from 0.1.0 to 0.2.0 ([#97](https://github.com/8xFF/atm0s-media-server/pull/97))

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-v0.1.0) - 2023-11-23

### Added
- whip/whep protocol, embed js samples ([#76](https://github.com/8xFF/atm0s-media-server/pull/76))

### Fixed
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))

### Other
- remove publish = false ([#73](https://github.com/8xFF/atm0s-media-server/pull/73))
- rename package. added release-plz for auto manage version ([#70](https://github.com/8xFF/atm0s-media-server/pull/70))
- Update Rust crate clap to 4.4.8 ([#53](https://github.com/8xFF/atm0s-media-server/pull/53))
- Update Rust crate clap to 4.4.7 ([#23](https://github.com/8xFF/atm0s-media-server/pull/23))
- simple rtmp server with SAN I/O style ([#40](https://github.com/8xFF/atm0s-media-server/pull/40))
- 17 integrate with bluesea sdn v4 ([#18](https://github.com/8xFF/atm0s-media-server/pull/18))
- cargo fmt
- break between media-server and transports ([#12](https://github.com/8xFF/atm0s-media-server/pull/12))
