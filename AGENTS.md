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

# Development workflow (lint-fix, test, build)
just dev

# Quick check (lint and build, no tests)
just check
```

### Running Examples

```bash
just example-doctor  # System diagnostics
just example-net     # P2P discovery
just example-rtsp    # RTSP server
```

### Debugging

Enable debug logging:
```bash
just debug doctor
just debug daemon
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
- [ ] No lint warnings: `just lint`
- [ ] Code formatted: `just fmt`
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if significant change)
- [ ] Pre-commit hooks pass: `just pre-commit`

## Troubleshooting

### Build Errors

```bash
just clean
just build
```

### Test Failures

```bash
just test-verbose
just test-integration
```

### Clippy Warnings

```bash
just lint-fix
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

## H.265/HEVC Support Notes

### Current Status

- **H.264** works with hardware encoding (`vah264enc`) and software encoding (`x264enc`)
- **H.265** HDCP handshake implemented with multi-approach verification
- TV sends r_rx before H_prime, suggesting pre-computed H_prime
- Multi-approach verification tries all possible IV/message format combinations

### HDCP 2.x Implementation Progress

1. **Handshake Messages Working**
   - AKE_Init → AKE_Send_Cert → AKE_No_Stored_Km → r_rx → H_prime → Pairing_Info → LC_Init → L_prime → SKE_Send_Eks
   - Full handshake observed in packet captures

2. **Key Findings**
   - IV construction: `r_tx || r_rx[0..7] || counter` for HDCP 2.2+ (NOT full r_rx)
   - Counter is in byte 15, XORed with 0x01 for second block
   - RSA-OAEP with SHA-1, empty label
   - Kd derivation uses AES-ECB to encrypt IV blocks
   - TV sends r_rx (0x06) BEFORE H_prime (0x07), unusual per spec

3. **Multi-Approach Verification (NEW)**
   - Tries all combinations of IV and message formats
   - HDCP 2.2 IV (with r_rx) + HDCP 2.2 message (r_tx || repeater || receiver_id)
   - HDCP 2.2 IV + HDCP 2.0 message (just r_tx)
   - HDCP 2.0 IV (no r_rx) + HDCP 2.2 message
   - HDCP 2.0 IV + HDCP 2.0 message
   - Message formats with r_rx included
   - Alternative IV format (r_tx || counter || r_rx)
   - Stores verified Kd for consistent L_prime and SKE computation

4. **Key Derivation**
   - Kd = AES-ECB(Km, r_tx || r_rx[0..7] || 0x00) || AES-ECB(Km, r_tx || r_rx[0..7] || 0x01)
   - Kd2 for SKE: key = Km XOR (0x00..00 || r_n), IV with counter 0x02
   - H_prime message format for HDCP 2.2+: r_tx || repeater_bit || receiver_id
   - L_prime key: Kd XOR r_rx (last 8 bytes), message: r_n

### Files to Review

- `crates/daemon/src/lib.rs`:
  - Lines 1311-1378: `verify_h_prime_multi_approach()` - multi-format H_prime verification
  - Lines 1380-1446: `verify_l_prime_multi_approach()` - multi-format L_prime verification
  - Lines 1498-1559: `compute_hdcp_kd()` and `compute_hdcp_kd_alt_iv()` - Kd derivation
  - Lines 1561-1588: `compute_hdcp_kd2()` - Kd2 for SKE
  - Lines 1252-1286: `send_hdcp_ske_send_eks_with_kd()` - SKE with verified Kd
- `crates/stream/src/lib.rs` lines 793-870 for encryption setup
- `docs/HDCP_TEST_VECTORS.md` for test vectors

### Next Steps

1. Test with real TV to see which format matches H_prime
2. If no format matches, investigate stored pairing state (AKE_Stored_Km)
3. Store Km after successful pairing for future connections
4. Verify encrypted H.265 stream is correctly decrypted by TV
