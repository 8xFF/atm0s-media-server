# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-cluster-local-v0.1.0) - 2023-11-23

### Fixed
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))
- fixing for working with origin str0m

### Other
- remove publish = false ([#73](https://github.com/8xFF/atm0s-media-server/pull/73))
- rename package. added release-plz for auto manage version ([#70](https://github.com/8xFF/atm0s-media-server/pull/70))
- simple rtmp server with SAN I/O style ([#40](https://github.com/8xFF/atm0s-media-server/pull/40))
- 17 intergrate with bluesea sdn v4 ([#18](https://github.com/8xFF/atm0s-media-server/pull/18))
- implement sim-svc logic. TODO: finish test
- added simulcast, svc parse
- handle share/unshare. mute/unmute
- reduce manual init some variable in cluster test
- added readme.md and more test in endpoint
- first working with some hack in sdk: receiver track msid should be audio_xxx or audio_xxx format
- test cluster local
- added more flow in track, req_res
- add http for temp working with whip
- first structure
