# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/8xFF/atm0s-media-server/releases/tag/atm0s-media-server-connector-v0.1.0) - 2025-02-08

### Added

- automatic SDN config with node-api and local_ip (#455)
- record compose (#448)
- multi tenancy  (#433)
- event hook (#420)
- media record  (#329)
- connector (#316)

### Fixed

- some time connector handle duplicate incorrect, ensure it success (#480)
- migration failed with mysql database (#456)
- crash assert on destroy (#449)
- postgresql query error (#419)
- add missing pagination to connector log apis (#363)
- api missing data (#355)
- wrong between created_at and session_id in sessions api (#352)
- build warnings and clippy warnings (#328)

### Other

- cleanup deps and fix for release-plz (#496)
- update metadata for packages (#492)
- update deps (#422)
- update docs installation (#343)
