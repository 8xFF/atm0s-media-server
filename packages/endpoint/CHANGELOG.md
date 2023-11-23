# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-endpoint-v0.1.0) - 2023-11-23

### Added
- whip/whep protocol, embed js samples ([#76](https://github.com/8xFF/atm0s-media-server/pull/76))

### Fixed
- auto build releases and publish docker ([#84](https://github.com/8xFF/atm0s-media-server/pull/84))
- fixing for working with origin str0m
- fixing bitrate-allocator overwrite with min-spatial and min-temporal

### Other
- remove publish = false ([#73](https://github.com/8xFF/atm0s-media-server/pull/73))
- rename package. added release-plz for auto manage version ([#70](https://github.com/8xFF/atm0s-media-server/pull/70))
- update sdn, str0m. implement remb. fixed single video slow bootstrap ([#68](https://github.com/8xFF/atm0s-media-server/pull/68))
- 17 intergrate with bluesea sdn v4 ([#18](https://github.com/8xFF/atm0s-media-server/pull/18))
- dynamic payload type from remote ([#16](https://github.com/8xFF/atm0s-media-server/pull/16))
- break between media-server and transports ([#12](https://github.com/8xFF/atm0s-media-server/pull/12))
- implement bitrate bwe current and desired. updated to newest str0m
- added vp9 test
- endpoint vp8_sim: test cases
- working with cluster room event scope: Auto or Manual
- fast start video
- implement vp8 simulcast packet filter with picture_id, tl0x rewrite
- implement sim-svc logic. TODO: finish test
- added simulcast, svc parse
- handle share/unshare. mute/unmute
- added publish/unpublish with webrtc
- refactor something: split webrtc session logic
- reduce manual init some variable in cluster test
- added readme.md and more test in endpoint
- added more test in webrtc transport utils
- first working with some hack in sdk: receiver track msid should be audio_xxx or audio_xxx format
- test cluster local
- rename for fixing warn
- added more flow in track, req_res
- add http for temp working with whip
- first structure
