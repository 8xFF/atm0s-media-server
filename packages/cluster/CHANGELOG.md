# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-cluster-v0.2.0...atm0s-media-server-cluster-v0.2.1) - 2024-01-26

### Fixed
- some sdk http apis enum between serde and poem-openapi ([#195](https://github.com/8xFF/atm0s-media-server/pull/195))
- webrtc sdk apis json parse error ([#193](https://github.com/8xFF/atm0s-media-server/pull/193))

### Other
- update metric-dashboard and poem deps ([#190](https://github.com/8xFF/atm0s-media-server/pull/190))

## [0.2.0](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-cluster-v0.1.0...atm0s-media-server-cluster-v0.2.0) - 2024-01-24

### Added
- gateway global ([#185](https://github.com/8xFF/atm0s-media-server/pull/185))
- sip transport and hooks ([#167](https://github.com/8xFF/atm0s-media-server/pull/167))
- allow run https self-signed cert for testing with remote server ([#175](https://github.com/8xFF/atm0s-media-server/pull/175))
- node info endpoint ([#151](https://github.com/8xFF/atm0s-media-server/pull/151))
- connector external event log - protobuf ([#132](https://github.com/8xFF/atm0s-media-server/pull/132))
- implement secure with static key JWT, update atm0s-sdn, update readme ([#129](https://github.com/8xFF/atm0s-media-server/pull/129))
- audio mix-minus and allow subscribe multi sources ([#126](https://github.com/8xFF/atm0s-media-server/pull/126))
- gateway and refactor media-server ([#106](https://github.com/8xFF/atm0s-media-server/pull/106))

### Fixed
- *(deps)* update rust crate atm0s-sdn to 0.1.8 ([#162](https://github.com/8xFF/atm0s-media-server/pull/162))
- whip/whep sdp patch with client ices failed [#176](https://github.com/8xFF/atm0s-media-server/pull/176) ([#179](https://github.com/8xFF/atm0s-media-server/pull/179))
- missing dashboard in gateway and live sessions not update when session ended ([#111](https://github.com/8xFF/atm0s-media-server/pull/111))
- wrong typos cause publish error ([#93](https://github.com/8xFF/atm0s-media-server/pull/93))

### Other
- rename token terms ([#174](https://github.com/8xFF/atm0s-media-server/pull/174))
- restructure cargo workspace deps and fix [#122](https://github.com/8xFF/atm0s-media-server/pull/122) ([#125](https://github.com/8xFF/atm0s-media-server/pull/125))
- Feat connector server ([#120](https://github.com/8xFF/atm0s-media-server/pull/120))
- release ([#88](https://github.com/8xFF/atm0s-media-server/pull/88))
