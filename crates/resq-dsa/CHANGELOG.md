# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/resq-software/crates/releases/tag/0.1.2) - 2026-04-13

### Added

- Repo hardening — RAII terminal guard, cargo-deny, 67 new tests ([#40](https://github.com/resq-software/crates/pull/40))
- Add comprehensive examples directory with runnable demos ([#38](https://github.com/resq-software/crates/pull/38))

### Changed

- Rename crate directories to match package names, fix stale references, and add comprehensive docs

### Fixed

- Critical correctness bugs + 39 new tests ([#39](https://github.com/resq-software/crates/pull/39))



## [0.1.1](https://github.com/resq-software/crates/compare/resq-dsa-v0.1.0...resq-dsa-v0.1.1) - 2026-03-27

### Fixed

- allow pedantic clippy lints in complexity test file
- resolve clippy decimal_bitwise_operands and mark timing tests as ignored

### Other

- add algorithmic complexity verification tests
