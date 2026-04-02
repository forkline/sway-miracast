# Opencode Guidelines for swaybeam

This file provides instructions for opencode (AI assistant) when assisting with the swaybeam project.

## Overview

swaybeam is a Miracast source implementation for wlroots-based Wayland compositors. It enables wireless display streaming from Linux systems to Miracast-compatible TVs, monitors, and projectors using Wi-Fi Direct.

## Project Structure

The project is organized as a Rust workspace with the following crates:

- `swaybeam-doctor` - System capability checks and validation
- `swaybeam-capture` - Screen capture via xdg-desktop-portal-wlr and PipeWire
- `swaybeam-stream` - GStreamer video encoding pipeline
- `swaybeam-net` - Wi-Fi Direct P2P networking
- `swaybeam-rtsp` - WFD RTSP protocol implementation
- `swaybeam-daemon` - Session orchestration
- `swaybeam-cli` - Command-line interface

## When Assisting with Development

### 1. Code Style

- Follow Rust naming conventions
- Use `#[derive(Debug, Clone)]` for most structs
- Implement `Display` for enums that represent status/error types
- Use `thiserror` for error types
- Document public APIs with `///` comments
- Write unit tests for new functionality

### 2. Commit Messages

Use conventional commit format:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `test:` - Test additions/changes
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

Example: `feat(capture): add PipeWire stream configuration`

### 3. Testing Requirements

All code should include:
- Unit tests for public functions
- Integration tests for cross-crate functionality
- Documentation tests for examples

Run tests with:
```bash
just test
# or
cargo test --workspace
```

### 4. Before Submitting Changes

Run these checks:
```bash
just lint          # Format + clippy
just test          # All tests
just pre-commit    # Pre-commit hooks
```

### 5. Release Process

1. Update version in `Cargo.toml`
2. Update `Cargo.lock`: `cargo update -p swaybeam`
3. Update changelog: `just update-changelog`
4. Commit: `git commit -m "release: Version X.Y.Z"`
5. Tag and release are automatic after merge to main

## Development Workflow

### Starting New Work

```bash
# Create feature branch
git checkout -b feat/my-feature

# Make changes and test
just test

# Run linting
just lint-fix
```

### Running Examples

```bash
# System diagnostics
cargo run --example check_system -p swaybeam-doctor

# P2P discovery
cargo run --example discover_and_connect -p swaybeam-net

# RTSP server
cargo run --example basic_server -p swaybeam-rtsp
```

### Debugging

Enable debug logging:
```bash
RUST_LOG=debug cargo run --example check_system -p swaybeam-doctor
```

## Common Tasks

### Adding a New Crate

1. Create directory: `mkdir -p crates/new-crate/src`
2. Create `Cargo.toml`:
   ```toml
   [package]
   name = "swaybeam-new-crate"
   version.workspace = true
   edition.workspace = true

   [dependencies]
   anyhow.workspace = true
   thiserror.workspace = true
   ```
3. Add to workspace in root `Cargo.toml`
4. Create `src/lib.rs` with public API

### Adding a New Check to Doctor

1. Add function in `crates/doctor/src/lib.rs`:
   ```rust
   pub fn check_new_thing() -> anyhow::Result<CheckResult> {
       // Implementation
   }
   ```
2. Add to `check_all()` function
3. Add field to `Report` struct
4. Add test in `#[cfg(test)]` module
5. Update `Report::print()` method

### Extending RTSP Protocol

1. Add new WFD parameter to `WfdCapabilities` struct
2. Add parser in `WfdCapabilities::set_parameter()`
3. Add getter in `WfdCapabilities::get_parameter()`
4. Update state machine if needed
5. Add tests

## Testing Checklist

Before submitting PR:
- [ ] All tests pass: `just test`
- [ ] No clippy warnings: `just clippy`
- [ ] Code formatted: `just fmt`
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if significant change)
- [ ] Pre-commit hooks pass: `just pre-commit`

## Troubleshooting

### Build Errors

```bash
# Clean and rebuild
just clean
cargo build
```

### Test Failures

```bash
# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_name -- --nocapture
```

### Clippy Warnings

```bash
# Auto-fix what's possible
just clippy-fix
```

## Architecture Notes

### Data Flow

1. User runs CLI command
2. Daemon orchestrates the session
3. Doctor validates system
4. Net discovers and connects to sink
5. RTSP negotiates capabilities
6. Capture starts screen capture
7. Stream encodes and transmits

### Error Handling

Use `anyhow::Result` for fallible operations:
```rust
pub fn do_something() -> anyhow::Result<()> {
    // ...
}
```

Use `thiserror::Error` for library errors:
```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("Something failed: {0}")]
    Failed(String),
}
```

### Async vs Sync

- Use `tokio` for I/O-bound operations (network, file)
- Use sync for CPU-bound or quick operations
- Doctor checks are synchronous (no async needed)

## Dependencies

Key dependencies:
- `tokio` - Async runtime
- `anyhow` - Error handling
- `thiserror` - Error types
- `tracing` - Logging
- `parking_lot` - Synchronization

When adding dependencies:
1. Add to workspace `Cargo.toml` if shared
2. Add version constraint (e.g., `"1.0"`)
3. Run `cargo update` to update lock file
4. Document why dependency is needed
