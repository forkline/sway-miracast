# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.1.0](https://github.com/forkline/swaybeam/tree/v0.1.0) - 2026-04-02

### Added

- Initial project structure with Rust workspace ([72b227e](https://github.com/forkline/swaybeam/commit/72b227e))
- Doctor crate with system capability checks ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Capture crate with PipeWire/xdg-desktop-portal API ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Stream crate with GStreamer pipeline types ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Net crate with P2P discovery via nmcli ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- RTSP crate with WFD protocol implementation ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Daemon crate for session orchestration ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- CLI crate with command-line interface ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))

### Documentation

- Architecture documentation ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Protocol documentation for WFD/RTSP ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Testing guide with system requirements ([6096857](https://github.com/forkline/swaybeam/commit/6096857))
- README with installation and usage instructions ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))

### Testing

- Unit tests for all crates (51 tests) ([1442aab](https://github.com/forkline/swaybeam/commit/1442aab))
- Integration tests for cross-crate functionality ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- Doc tests for public API examples ([fdc5894](https://github.com/forkline/swaybeam/commit/fdc5894))
- System test script for environment validation ([6096857](https://github.com/forkline/swaybeam/commit/6096857))
