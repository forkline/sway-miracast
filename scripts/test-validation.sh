#!/bin/bash
# Test script to validate Miracast implementation
# Run with: just test-validation

set -e

echo "╔═══════════════════════════════════════════╗"
echo "║   Swaybeam Validation Test Suite          ║"
echo "║   Testing against WFD specification       ║"
echo "╚═══════════════════════════════════════════╝"
echo

# Test 1: Protocol Compliance
echo "📋 Test 1: RTSP/WFD Protocol Compliance"
echo "=========================================="
cargo test --package swaybeam-rtsp --test spec_compliance -- --nocapture --test-threads=1
echo "✓ Protocol compliance tests passed"
echo

# Test 2: Session Simulation
echo "🎬 Test 2: Full Session Simulation"
echo "===================================="
cargo test --package swaybeam-daemon --test session_simulation -- --nocapture --test-threads=1
echo "✓ Session simulation tests passed"
echo

# Test 3: Network Layer
echo "🌐 Test 3: Network Layer Tests"
echo "================================"
cargo test --package swaybeam-net --test network_tests -- --nocapture --test-threads=1
echo "✓ Network layer tests passed"
echo

# Test 4: RTSP Protocol Tests
echo "📡 Test 4: RTSP Protocol Tests"
echo "================================"
cargo test --package swaybeam-rtsp --test protocol_tests -- --nocapture --test-threads=1
echo "✓ RTSP protocol tests passed"
echo

# Test 5: Integration Tests
echo "🔗 Test 5: Integration Tests"
echo "=============================="
cargo test --package swaybeam-rtsp --test integration_tests -- --nocapture --test-threads=1
echo "✓ Integration tests passed"
echo

# Summary
echo "╔═══════════════════════════════════════════╗"
echo "║   All Validation Tests Passed! ✓          ║"
echo "╚═══════════════════════════════════════════╝"
echo
echo "Next steps:"
echo "  1. Test with mock server: cargo run --example mock_sink_server"
echo "  2. Run integration tests with real services: just test-integration"
echo "  3. Test on real hardware with Miracast display"
echo
