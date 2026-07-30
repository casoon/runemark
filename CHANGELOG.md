# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Public presentation types are now non-exhaustive so Runemark can extend its
  semantic vocabulary without a breaking release. This is a breaking change
  for downstream exhaustive matches and struct literals.
- Report internals are split into model and renderer modules while preserving
  the public `runemark::report` API.

### Fixed

- Detailed reports now explicitly identify findings that were not provided in
  the report data instead of implying a complete list.

## [0.1.1] - 2026-07-29

### Fixed

- Report rendering now accepts trait-object writers without an intermediate buffer.

## [0.1.0] - 2026-07-29

### Added

- Initial `Console`, `ColorMode`, and semantic `Tone` API.
- TTY-aware color policy with `NO_COLOR` support.
