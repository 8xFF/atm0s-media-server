# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.2](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-transport-rtpengine-v0.1.0-alpha.1...atm0s-media-server-transport-rtpengine-v0.1.0-alpha.2) - 2025-02-08

### Other

- release 0.2.0-alpha.2 (#504)

## [0.1.0-alpha.1](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-transport-rtpengine-v0.1.0-alpha.1) - 2025-02-08

### Added

- automatic SDN config with node-api and local_ip (#455)
- multi tenancy  (#433)
- rtp transport with HTTP APIs (#424)
- transport for SIP with rtpengine protocol  (#359)

### Fixed

- increase rtp timeout to 3 minutes (#482)
- wrong usage of smallmap cause server crash. switched to indexmap (#457)
- crash assert on destroy (#449)
- endpoint internal clean up crash (#447)
- rtpengine generated sdp missing PCMA codec (#430)

### Other

- update version for release-plz (#500)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
