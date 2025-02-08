# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.1](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-gateway-v0.1.0-alpha.1) - 2025-02-08

### Added

- switch to manual2 discovery (#486)
- multi tenancy  (#433)
- transport for SIP with rtpengine protocol  (#359)
- media record  (#329)
- console API (#311)
- api gateway and session token for securing cluster (#292)

### Fixed

- media-gateway rtpengine missing clear timeout (#470)
- crash assert on destroy (#449)
- route restart-ice to another media node if the current one is down (#410)
- build warnings and clippy warnings (#328)
- media gateway wrong cpu and memory compare (#299)
- typos and clippy warns (#296)

### Other

- update version for release-plz (#500)
- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- config zone id node id media port, get console lists (#417)
- cargo update and some libs (#381)
- run cargo update (#309)
- Feat ping with node usage ([#298](https://github.com/8xFF/atm0s-media-server/pull/298))
- registry store (#297)
