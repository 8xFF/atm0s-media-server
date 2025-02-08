# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2](https://github.com/8xFF/atm0s-media-server/compare/atm0s-media-server-utils-v0.2.1...atm0s-media-server-utils-v0.2.2) - 2025-02-08

### Added

- move frontend to inside (#469)
- automatic SDN config with node-api and local_ip (#455)
- transport for SIP with rtpengine protocol  (#359)
- media record  (#329)
- connector (#316)

### Fixed

- some clippy warns (#490)
- wrong usage of smallmap cause server crash. switched to indexmap (#457)
- crash assert on destroy (#449)
- endpoint internal clean up crash (#447)
- route restart-ice to another media node if the current one is down (#410)
- typos and clippy warns (#296)

### Other

- update version for release-plz (#497)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- config zone id node id media port, get console lists (#417)
- fix clippy actions workflow and add cargo-fmt action (#353)
- more clippy fixes (#349)
- Feat svc simulcast ([#266](https://github.com/8xFF/atm0s-media-server/pull/266))
- BREAKING CHANGE: switching to sans-io-runtime ([#257](https://github.com/8xFF/atm0s-media-server/pull/257))
