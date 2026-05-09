# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/resq-software/crates/releases/tag/0.3.0) - 2026-05-04

### Added

- *(cli)* Scan/tui command groups (Stage 3) ([#75](https://github.com/resq-software/crates/pull/75))
- *(cli)* Global flags + shell completions + richer help (Stage 1) ([#74](https://github.com/resq-software/crates/pull/74))
- Add resq commit with multi-provider AI commit message generation ([#70](https://github.com/resq-software/crates/pull/70))
- *(cli)* Extract `resq format` subcommand from pre_commit orchestrator ([#62](https://github.com/resq-software/crates/pull/62))
- *(cli)* Consolidate hook surface under `hooks`; deprecate `dev` paths ([#60](https://github.com/resq-software/crates/pull/60))
- *(copyright)* Detect author mismatch, not just license mismatch ([#59](https://github.com/resq-software/crates/pull/59))

### CI

- Migrate to reusable rust-ci, remove clippy.yml + deny.yml ([#71](https://github.com/resq-software/crates/pull/71))

### Changed

- *(cli)* Remove orphaned lqip / cost / tree-shake commands ([#61](https://github.com/resq-software/crates/pull/61))


# Changelog

All notable changes to this project will be documented in this file.



## [0.2.6](https://github.com/resq-software/crates/releases/tag/0.2.6) - 2026-04-14



## [0.2.5](https://github.com/resq-software/crates/releases/tag/0.2.5) - 2026-04-14

### Added

- *(resq-cli)* `hooks` subcommand + `dev scaffold-local-hook` ([#48](https://github.com/resq-software/crates/pull/48))
- *(resq-cli)* Scaffold hooks from embedded templates + release workflow ([#46](https://github.com/resq-software/crates/pull/46))
- Add comprehensive examples directory with runnable demos ([#38](https://github.com/resq-software/crates/pull/38))

### Changed

- Rename crate directories to match package names, fix stale references, and add comprehensive docs

### Testing

- *(resq-cli)* Integration tests + fix install-hooks partial-layout bug ([#49](https://github.com/resq-software/crates/pull/49))



## [0.2.5](https://github.com/resq-software/crates/releases/tag/0.2.5) - 2026-04-13

### Added

- Add comprehensive examples directory with runnable demos ([#38](https://github.com/resq-software/crates/pull/38))

### Changed

- Rename crate directories to match package names, fix stale references, and add comprehensive docs



## [0.2.4](https://github.com/resq-software/crates/compare/resq-cli-v0.2.3...resq-cli-v0.2.4) - 2026-04-08

### Other

- apply PR review feedback — glob members, fix deploy-cli broken links, sync CLAUDE.md ([#36](https://github.com/resq-software/crates/pull/36))

## [0.2.3](https://github.com/resq-software/crates/compare/resq-cli-v0.2.2...resq-cli-v0.2.3) - 2026-03-27

### Other

- update repo references after rename cli → crates

## [0.2.2](https://github.com/resq-software/crates/compare/resq-cli-v0.2.1...resq-cli-v0.2.2) - 2026-03-17

### Other

- address cli review follow-ups
- optimize rust workspace configuration
- remove gitleaks binary and update configuration files

## [0.2.1](https://github.com/resq-software/crates/compare/resq-cli-v0.2.0...resq-cli-v0.2.1) - 2026-03-15
## [0.2.1](https://github.com/resq-software/crates/compare/resq-cli-v0.2.0...resq-cli-v0.2.1) - 2026-03-15

### Fixed

- *(ci)* replace gitleaks-action with free CLI binary — org license not configured
<!--
  Copyright 2026 ResQ

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->


## [0.2.0](https://github.com/resq-software/crates/compare/resq-cli-v0.1.0...resq-cli-v0.2.0) - 2026-03-15

### Other

- add dev tooling — hooks, skills, agents, codecov, dependabot
