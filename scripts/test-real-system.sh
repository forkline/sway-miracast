#!/bin/bash
# Test script for real system integration

set -e

echo "=== Checking System Requirements ==="

# Check PipeWire
if pgrep -x pipewire > /dev/null; then
    echo "✓ PipeWire is running"
else
    echo "✗ PipeWire is not running"
fi

# Check GStreamer
if gst-inspect-1.0 x264enc > /dev/null 2>&1; then
    echo "✓ GStreamer x264enc available"
else
    echo "✗ GStreamer x264enc not available (need gst-plugins-ugly)"
fi

# Check NetworkManager
if systemctl is-active --quiet NetworkManager; then
    echo "✓ NetworkManager is running"
else
    echo "✗ NetworkManager is not running"
fi

# Check xdg-desktop-portal
if pgrep -f xdg-desktop-portal > /dev/null; then
    echo "✓ xdg-desktop-portal is running"
else
    echo "✗ xdg-desktop-portal is not running"
fi

echo ""
echo "=== Running Integration Tests ==="

# Build first
cargo build --workspace

# Run integration tests (marked with #[ignore])
echo ""
echo "Running integration tests (requires real services)..."
cargo test --test integration_real -- --ignored --nocapture

echo ""
echo "=== Integration Tests Complete ==="
