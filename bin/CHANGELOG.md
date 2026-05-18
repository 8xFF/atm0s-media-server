# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0-alpha.9](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.8...v0.2.0-alpha.9) - 2026-05-18

### Added

- custom s3 presign lib for compatible with recording ([#547](https://github.com/8xFF/atm0s-media-server/pull/547))

### Fixed

- webm seek issue ([#550](https://github.com/8xFF/atm0s-media-server/pull/550))

### Other

- *(deps)* update dependency js-cookie to v3.0.7 ([#551](https://github.com/8xFF/atm0s-media-server/pull/551))
- *(deps)* update dependency axios to v1.15.2 [security] ([#488](https://github.com/8xFF/atm0s-media-server/pull/488))
- *(deps)* update dependency @types/lodash to v4.17.24 ([#489](https://github.com/8xFF/atm0s-media-server/pull/489))
- *(deps)* update dependency @vitejs/plugin-react to v4.7.0 ([#512](https://github.com/8xFF/atm0s-media-server/pull/512))
- *(deps)* update dependency @radix-ui/react-icons to v1.3.2 ([#537](https://github.com/8xFF/atm0s-media-server/pull/537))
- *(deps)* update dependency lodash to v4.18.1 [security] ([#535](https://github.com/8xFF/atm0s-media-server/pull/535))
- *(deps)* update dependency class-variance-authority to v0.7.1 ([#538](https://github.com/8xFF/atm0s-media-server/pull/538))
- *(deps)* update dependency dayjs to v1.11.20 ([#539](https://github.com/8xFF/atm0s-media-server/pull/539))
- *(deps)* update dependency postcss to v8.5.10 [security] ([#549](https://github.com/8xFF/atm0s-media-server/pull/549))
- *(deps)* update dependency react-resizable-panels to v2.1.9 ([#546](https://github.com/8xFF/atm0s-media-server/pull/546))

## [0.2.0-alpha.8](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.7...v0.2.0-alpha.8) - 2026-04-23

### Fixed

- avoid crash when media pkt after remote track ended ([#542](https://github.com/8xFF/atm0s-media-server/pull/542))

## [0.2.0-alpha.7](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.6...v0.2.0-alpha.7) - 2025-03-02

### Fixed

- vnet should stick on same worker for avoiding actor missmatch error ([#519](https://github.com/8xFF/atm0s-media-server/pull/519))

## [0.2.0-alpha.6](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.5...v0.2.0-alpha.6) - 2025-03-02

### Fixed

- wrong worker setting cause cross nodes stream subscribe failed ([#517](https://github.com/8xFF/atm0s-media-server/pull/517))

### Other

- update Cargo.lock dependencies
- migrate to tailwindcss v4, update layout, router ([#514](https://github.com/8xFF/atm0s-media-server/pull/514))

## [0.2.0-alpha.5](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.4...v0.2.0-alpha.5) - 2025-02-27

### Other

- update atm0s-sdn for fixing network unstable issue (#513)
- update Cargo.lock dependencies

## [0.2.0-alpha.4](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.3...v0.2.0-alpha.4) - 2025-02-26

### Added

- simple nodes visualization in console (#509)

## [0.2.0-alpha.3](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.2...v0.2.0-alpha.3) - 2025-02-08

### Other

- release 0.2.0-alpha.2 (#504)

## [0.2.0-alpha.2](https://github.com/8xFF/atm0s-media-server/compare/v0.2.0-alpha.1...v0.2.0-alpha.2) - 2025-02-08

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
- switch to manual2 discovery (#486)
- automatic SDN config with node-api and local_ip (#455)
- standlone server (#454)
- record compose (#448)
- event hook (#420)
- transport for SIP with rtpengine protocol  (#359)
- console API (#311)

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
- missing config connector agent service which caused missing peer logs (#405)
- unsuccessful bind addr cause crash media node (#369)
- update atm0s-sdn for fix media-node failed to register gateway after restart caused by broadcast register message was rejected by history cache logic (#337)
- increase rtp timeout to 3 minutes (#482)
- rtpengine generated sdp missing PCMA codec (#430)

### Other

- update version for release-plz (#500)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- ename peer's userdata to extra_data for avoid miss-understand (#386)
- fix clippy actions workflow and add cargo-fmt action (#353)
- more clippy fixes (#349)
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
- config zone id node id media port, get console lists (#417)
- switched to internal deps from crate.io (#367)
- Feat ping with node usage ([#298](https://github.com/8xFF/atm0s-media-server/pull/298))
