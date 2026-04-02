# sway-miracast Justfile
# Run `just --list` to see available commands

PROJECT_VERSION := `sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1`

# Show available commands
default:
    just --list

# Build in release mode
build:
    cargo build --release

# Run all tests
test:
    cargo test --workspace

# Format and lint
lint:
    cargo fmt --check
    cargo clippy --workspace -- -D warnings

# Fix linting issues
lint-fix:
    cargo fmt
    cargo clippy --workspace --fix --allow-dirty

# Run system checks
doctor:
    cargo run --release --bin miracast -- doctor

# Run integration tests (requires real services)
test-integration:
    cargo test --workspace -- --ignored --nocapture

# Install pre-commit hooks
pre-commit:
    pre-commit install

# Clean build artifacts
clean:
    cargo clean

# Build release tarball
release:
    cargo build --release
    tar -czf miracast-{{PROJECT_VERSION}}.tar.gz -C target/release miracast
