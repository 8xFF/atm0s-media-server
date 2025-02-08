# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/media-server-record-v0.1.0) - 2025-02-08

### Added

- automatic SDN config with node-api and local_ip (#455)
- add duration_ms to compose record result (#451)
- record compose (#448)
- multi tenancy  (#433)
- convert record to separated media files and push to s3 (#351)
- media record  (#329)

### Fixed

- handle video orientation from webrtc-extension (#452)
- crash assert on destroy (#449)
- build release with github action (#340)
- update atm0s-sdn for fix media-node failed to register gateway after restart caused by broadcast register message was rejected by history cache logic (#337)

### Other

- update metadata for packages (#492)
- switch rusty-s3 to crates instead of git (#491)
- fix clippy actions workflow and add cargo-fmt action (#353)
- more clippy fixes (#349)
