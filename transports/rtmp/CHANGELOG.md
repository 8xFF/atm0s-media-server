# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-transport-rtmp-v0.2.0...atm0s-media-server-transport-rtmp-v0.2.1) - 2024-01-26

### Other
- updated the following local packages: atm0s-media-server-utils, atm0s-media-server-transport

## [0.2.0](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-transport-rtmp-v0.1.0...atm0s-media-server-transport-rtmp-v0.2.0) - 2024-01-24

### Added
- sip transport and hooks ([#167](https://github.com/8xFF/atm0s-media-server/pull/167))
- implement secure with static key JWT, update atm0s-sdn, update readme ([#129](https://github.com/8xFF/atm0s-media-server/pull/129))
- gateway and refactor media-server ([#106](https://github.com/8xFF/atm0s-media-server/pull/106))

### Fixed
- *(deps)* update rust crate fdk-aac to 0.6.0 ([#186](https://github.com/8xFF/atm0s-media-server/pull/186))
- unused warn and local cluster aggregate bitrate ([#99](https://github.com/8xFF/atm0s-media-server/pull/99))

### Other
- restructure cargo workspace deps and fix [#122](https://github.com/8xFF/atm0s-media-server/pull/122) ([#125](https://github.com/8xFF/atm0s-media-server/pull/125))
- Feat connector server ([#120](https://github.com/8xFF/atm0s-media-server/pull/120))

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-transport-rtmp-v0.1.0) - 2023-11-23

### Fixed
- update deps version for avoiding *, updated atm0s-sdn to 0.1.1 ([#87](https://github.com/8xFF/atm0s-media-server/pull/87))
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))

### Other
- update xflv to 0.3.0 ([#48](https://github.com/8xFF/atm0s-media-server/pull/48))
- simple rtmp server with SAN I/O style ([#40](https://github.com/8xFF/atm0s-media-server/pull/40))
