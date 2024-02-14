# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4](https://github.com/8xFF/atm0s-media-server/compare/v0.1.3...v0.1.4) - 2024-02-14

### Added
- impls gateway logic as RFC-0003 ([#219](https://github.com/8xFF/atm0s-media-server/pull/219))

### Fixed
- webrtc rpc not working if sender and receiver is created after rpc arrivered ([#200](https://github.com/8xFF/atm0s-media-server/pull/200))

### Other
- *(deps)* bump clap from 4.4.18 to 4.5.0 ([#230](https://github.com/8xFF/atm0s-media-server/pull/230))
- added typos github actions ([#224](https://github.com/8xFF/atm0s-media-server/pull/224))
- update rust crate reqwest to 0.11.24 ([#203](https://github.com/8xFF/atm0s-media-server/pull/203))
- update rust crate atm0s-sdn to 0.1.9 ([#202](https://github.com/8xFF/atm0s-media-server/pull/202))
- update rust crate str0m to 0.1.1 ([#226](https://github.com/8xFF/atm0s-media-server/pull/226))

## [0.1.3](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-v0.1.2...atm0s-media-server-v0.1.3) - 2024-01-26

### Fixed
- webrtc sdk apis json parse error ([#193](https://github.com/8xFF/atm0s-media-server/pull/193))
- some sdk http apis enum between serde and poem-openapi ([#195](https://github.com/8xFF/atm0s-media-server/pull/195))

### Other
- update metric-dashboard and poem deps ([#190](https://github.com/8xFF/atm0s-media-server/pull/190))

## [0.1.2](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-v0.1.1...atm0s-media-server-v0.1.2) - 2024-01-25

### Added
- sip transport and hooks ([#167](https://github.com/8xFF/atm0s-media-server/pull/167))

### Fixed
- unused warn and local cluster aggregate bitrate ([#99](https://github.com/8xFF/atm0s-media-server/pull/99))
- wrong typos cause publish error ([#93](https://github.com/8xFF/atm0s-media-server/pull/93))
- update deps version for avoiding *, updated atm0s-sdn to 0.1.1 ([#87](https://github.com/8xFF/atm0s-media-server/pull/87))
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))

### Other
- fix release-plz config ([#188](https://github.com/8xFF/atm0s-media-server/pull/188))
- release server 0.1.1 ([#123](https://github.com/8xFF/atm0s-media-server/pull/123))
- restructure cargo workspace deps and fix [#122](https://github.com/8xFF/atm0s-media-server/pull/122) ([#125](https://github.com/8xFF/atm0s-media-server/pull/125))
- temporal set publish=false sip transport ([#94](https://github.com/8xFF/atm0s-media-server/pull/94))
- release ([#88](https://github.com/8xFF/atm0s-media-server/pull/88))
- rename package. added release-plz for auto manage version ([#70](https://github.com/8xFF/atm0s-media-server/pull/70))
- 9 incomplete sip server ([#52](https://github.com/8xFF/atm0s-media-server/pull/52))

## [0.1.1](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-v0.1.0...atm0s-media-server-v0.1.1) - 2024-01-24

### Added
- gateway global ([#185](https://github.com/8xFF/atm0s-media-server/pull/185))
- sip transport and hooks ([#167](https://github.com/8xFF/atm0s-media-server/pull/167))
- allow run https self-signed cert for testing with remote server ([#175](https://github.com/8xFF/atm0s-media-server/pull/175))
- connector with persistent queue  ([#161](https://github.com/8xFF/atm0s-media-server/pull/161))
- F32p2 conversion to from f32 ([#152](https://github.com/8xFF/atm0s-media-server/pull/152))
- node info endpoint ([#151](https://github.com/8xFF/atm0s-media-server/pull/151))
- connector external event log - protobuf ([#132](https://github.com/8xFF/atm0s-media-server/pull/132))
- implement secure with static key JWT, update atm0s-sdn, update readme ([#129](https://github.com/8xFF/atm0s-media-server/pull/129))
- audio mix-minus and allow subscribe multi sources ([#126](https://github.com/8xFF/atm0s-media-server/pull/126))
- gateway and refactor media-server ([#106](https://github.com/8xFF/atm0s-media-server/pull/106))
- auto or manual peer info subscribe ([#135](https://github.com/8xFF/atm0s-media-server/pull/135))

### Fixed
- *(deps)* update rust crate clap to 4.4.18 ([#134](https://github.com/8xFF/atm0s-media-server/pull/134))
- whip/whep sdp patch with client ices failed [#176](https://github.com/8xFF/atm0s-media-server/pull/176) ([#179](https://github.com/8xFF/atm0s-media-server/pull/179))
- *(deps)* update rust crate yaque to 0.6.6 ([#169](https://github.com/8xFF/atm0s-media-server/pull/169))
- missing dashboard in gateway and live sessions not update when session ended ([#111](https://github.com/8xFF/atm0s-media-server/pull/111))
- *(deps)* update rust crate atm0s-sdn to 0.1.8 ([#162](https://github.com/8xFF/atm0s-media-server/pull/162))
- wrong typos cause publish error ([#93](https://github.com/8xFF/atm0s-media-server/pull/93))
- *(deps)* update rust crate lz4_flex to 0.11.2 ([#165](https://github.com/8xFF/atm0s-media-server/pull/165))
- *(deps)* update rust crate quote to 1.0.35 ([#139](https://github.com/8xFF/atm0s-media-server/pull/139))
- *(deps)* update rust crate syn to 2.0.48 ([#128](https://github.com/8xFF/atm0s-media-server/pull/128))
- *(deps)* update rust crate syn to 2.0.42 ([#124](https://github.com/8xFF/atm0s-media-server/pull/124))
- *(deps)* update rust crate syn to 2.0.41 ([#110](https://github.com/8xFF/atm0s-media-server/pull/110))
- *(deps)* update rust crate local-ip-address to 0.5.7 ([#178](https://github.com/8xFF/atm0s-media-server/pull/178))
- doctests sdp patch to ices ([#181](https://github.com/8xFF/atm0s-media-server/pull/181))
- wrong track_id convert from random webrtc Mid ([#140](https://github.com/8xFF/atm0s-media-server/pull/140))
- webrtc stream missing info if sdk stop then create new with same name ([#100](https://github.com/8xFF/atm0s-media-server/pull/100))
- unused warn and local cluster aggregate bitrate ([#99](https://github.com/8xFF/atm0s-media-server/pull/99))
- *(deps)* update rust crate fdk-aac to 0.6.0 ([#186](https://github.com/8xFF/atm0s-media-server/pull/186))

### Other
- rename token terms ([#174](https://github.com/8xFF/atm0s-media-server/pull/174))
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
