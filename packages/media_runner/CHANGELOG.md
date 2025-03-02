# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.3](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-runner-v0.1.0-alpha.2...atm0s-media-server-runner-v0.1.0-alpha.3) - 2025-03-02

### Fixed

- wrong worker setting cause cross nodes stream subscribe failed ([#517](https://github.com/8xFF/atm0s-media-server/pull/517))

## [0.1.0-alpha.2](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-runner-v0.1.0-alpha.1...atm0s-media-server-runner-v0.1.0-alpha.2) - 2025-02-08

### Other

- release 0.2.0-alpha.2 (#504)

## [0.1.0-alpha.1](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-runner-v0.1.0-alpha.1) - 2025-02-08

### Added

- switch to manual2 discovery (#486)
- automatic SDN config with node-api and local_ip (#455)
- standlone server (#454)
- record compose (#448)
- multi tenancy  (#433)
- rtp transport with HTTP APIs (#424)
- event hook (#420)
- transport for SIP with rtpengine protocol  (#359)
- graceful disconnect with webrtc (#385)
- embedded userdata to token (#379)
- media record  (#329)
- connector (#316)
- console API (#311)
- audio mixer (#306)
- api gateway and session token for securing cluster (#292)
- webrtc sdk (#283)

### Fixed

- wrong usage of smallmap cause server crash. switched to indexmap (#457)
- crash assert on destroy (#449)
- missing config connector agent service which caused missing peer logs (#405)
- unsuccessful bind addr cause crash media node (#369)
- update atm0s-sdn for fix media-node failed to register gateway after restart caused by broadcast register message was rejected by history cache logic (#337)
- typos and clippy warns (#296)

### Other

- update version for release-plz (#500)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- config zone id node id media port, get console lists (#417)
- ename peer's userdata to extra_data for avoid miss-understand (#386)
- switched to internal deps from crate.io (#367)
- Feat ping with node usage ([#298](https://github.com/8xFF/atm0s-media-server/pull/298))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
