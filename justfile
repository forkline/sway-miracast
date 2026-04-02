# sway-miracast Justfile
# Run `just --list` to see available commands

CARGO_TARGET_DIR := "target"
CARGO_TARGET := "x86_64-unknown-linux-gnu"
PROJECT_VERSION := `sed -n 's/^version = "\(.*\)"/\1/p' ./Cargo.toml | head -n1`
PKG_BASE_NAME := "miracast-" + PROJECT_VERSION + "-" + CARGO_TARGET

# Show available commands
default:
    just --list

# Compile in release mode
build:
    cargo build --release

# Install pre-commit hooks
pre-commit-install:
    pre-commit install

# Run pre-commit on all files
pre-commit:
    pre-commit run --all-files

# Format Rust code
fmt:
    cargo fmt

# Check Rust code formatting
fmt-check:
    cargo fmt -- --check

# Run clippy linter
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run clippy with automatic fixes
clippy-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty -- -D warnings

# Run all linting checks (fmt + clippy)
lint: fmt-check clippy

# Run all linting with automatic fixes
lint-fix: fmt clippy-fix

# Run unit tests
test-unit:
    cargo test --lib

# Run integration tests
test-integration:
    cargo test --test '*'

# Run all tests
test-all:
    cargo test --workspace --all-features

# Run all tests (lint + unit + integration)
test: lint test-all

# Run system diagnostics
doctor:
    cargo run --example check_system -p miracast-doctor

# Run system test script
system-check:
    ./scripts/test-system.sh

# Automatically update changelog based on commits
update-changelog:
    git cliff -t v{{PROJECT_VERSION}} -u -p CHANGELOG.md

# Generate release artifacts
release:
    cargo build --release --all-features --target {{CARGO_TARGET}}
    tar -czf {{PKG_BASE_NAME}}.tar.gz -C {{CARGO_TARGET_DIR}}/{{CARGO_TARGET}}/release miracast
    @echo "Released in {{CARGO_TARGET_DIR}}/{{CARGO_TARGET}}/release/miracast"

# Generate SHA256 hash for release
release-hash:
    sha256sum {{PKG_BASE_NAME}}.tar.gz > {{PKG_BASE_NAME}}.tar.gz.sha256
    cat {{PKG_BASE_NAME}}.tar.gz.sha256

# Clean build artifacts
clean:
    cargo clean
    rm -f *.tar.gz

# Show project info
info:
    @echo "Project: sway-miracast"
    @echo "Version: {{PROJECT_VERSION}}"
    @echo "Target: {{CARGO_TARGET}}"

# Run all examples
run-examples:
    cargo run --example check_system -p miracast-doctor
    cargo run --example discover_and_connect -p miracast-net
    cargo run --example basic_server -p miracast-rtsp

# Generate documentation
docs:
    cargo doc --workspace --no-deps --open

# Check for security vulnerabilities
security:
    cargo audit

# Run benchmarks (if any)
bench:
    cargo bench

# Watch for changes and run tests
watch:
    cargo watch -x test