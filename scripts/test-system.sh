#!/bin/bash
# Quick test script for sway-miracast
# Run this to verify your system is ready for Miracast

set -e

echo "========================================"
echo "  sway-miracast System Test Script"
echo "========================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# 1. Check Rust
echo "Checking Rust toolchain..."
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    pass "Rust installed: $RUST_VERSION"
else
    fail "Rust not installed. Install from https://rustup.rs"
    exit 1
fi

# 2. Check Cargo
if command -v cargo &> /dev/null; then
    CARGO_VERSION=$(cargo --version)
    pass "Cargo installed: $CARGO_VERSION"
else
    fail "Cargo not installed"
    exit 1
fi

# 3. Check Sway
echo ""
echo "Checking Sway compositor..."
if [ -n "$SWAYSOCK" ]; then
    pass "SWAYSOCK environment variable set"
    if command -v swaymsg &> /dev/null && swaymsg -t get_version &> /dev/null; then
        SWAY_VERSION=$(swaymsg -t get_version | head -1)
        pass "Sway is running: $SWAY_VERSION"
    else
        warn "SWAYSOCK set but swaymsg not responding"
    fi
else
    warn "SWAYSOCK not set - not running under Sway?"
    if pgrep -x sway &> /dev/null; then
        warn "Sway process detected but SWAYSOCK not set"
    else
        fail "Sway not running"
    fi
fi

# 4. Check PipeWire
echo ""
echo "Checking PipeWire..."
if pgrep -x pipewire &> /dev/null; then
    pass "PipeWire daemon running"
else
    fail "PipeWire not running. Start with: systemctl --user start pipewire"
fi

if pgrep -x pipewire-pulse &> /dev/null || pgrep -x pipewire-media-session &> /dev/null; then
    pass "PipeWire session manager running"
else
    warn "PipeWire session manager not found"
fi

# 5. Check GStreamer
echo ""
echo "Checking GStreamer..."
if command -v gst-inspect-1.0 &> /dev/null; then
    GST_VERSION=$(gst-inspect-1.0 --version | head -1)
    pass "GStreamer installed: $GST_VERSION"
    
    # Check required plugins
    PLUGINS=("x264" "openh264" "h264parse" "rtph264pay")
    MISSING=()
    
    for plugin in "${PLUGINS[@]}"; do
        if gst-inspect-1.0 "$plugin" &> /dev/null; then
            pass "  Plugin: $plugin"
        else
            MISSING+=("$plugin")
        fi
    done
    
    if [ ${#MISSING[@]} -ne 0 ]; then
        fail "Missing GStreamer plugins: ${MISSING[*]}"
        echo "  Install with: sudo apt install gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav"
    fi
else
    fail "GStreamer not installed"
    echo "  Install with: sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good"
fi

# 6. Check NetworkManager
echo ""
echo "Checking NetworkManager..."
if pgrep -x NetworkManager &> /dev/null; then
    pass "NetworkManager daemon running"
else
    fail "NetworkManager not running"
    echo "  Start with: sudo systemctl start NetworkManager"
fi

if command -v nmcli &> /dev/null && nmcli general status &> /dev/null; then
    NM_STATUS=$(nmcli -t -f STATE general status)
    pass "NetworkManager status: $NM_STATUS"
else
    warn "nmcli not available or NetworkManager not responding"
fi

# 7. Check wpa_supplicant
echo ""
echo "Checking wpa_supplicant..."
if pgrep -x wpa_supplicant &> /dev/null; then
    pass "wpa_supplicant daemon running"
else
    if command -v wpa_supplicant &> /dev/null; then
        warn "wpa_supplicant binary found but not running"
        echo "  May need: sudo systemctl start wpa_supplicant"
    else
        fail "wpa_supplicant not installed"
    fi
fi

# 8. Check Wi-Fi P2P capability
echo ""
echo "Checking Wi-Fi P2P capability..."
if command -v iw &> /dev/null; then
    if iw list 2>/dev/null | grep -q "P2P"; then
        pass "Wi-Fi adapter supports P2P"
        iw list 2>/dev/null | grep -A 2 "P2P" | head -4
    else
        warn "No P2P-capable Wi-Fi adapter found"
        echo "  Check with: iw list | grep P2P"
    fi
else
    warn "iw tool not available, cannot check P2P capability"
fi

# 9. Check xdg-desktop-portal
echo ""
echo "Checking xdg-desktop-portal..."
if pgrep -x xdg-desktop-portal &> /dev/null; then
    pass "xdg-desktop-portal running"
    
    if pgrep -x xdg-desktop-portal-wlr &> /dev/null; then
        pass "xdg-desktop-portal-wlr backend running"
    else
        warn "xdg-desktop-portal-wlr not running (required for Sway)"
        echo "  Install and start: systemctl --user start xdg-desktop-portal-wlr"
    fi
else
    fail "xdg-desktop-portal not running"
    echo "  Start with: systemctl --user start xdg-desktop-portal"
fi

# 10. Check project builds
echo ""
echo "Checking project build..."
if cargo build --release 2>&1 | grep -q "Finished"; then
    pass "Project builds successfully"
else
    fail "Project build failed"
    cargo build --release 2>&1 | tail -10
fi

# 11. Run tests
echo ""
echo "Running project tests..."
if cargo test --workspace 2>&1 | grep -q "test result: ok"; then
    pass "All tests pass"
else
    warn "Some tests failed"
    cargo test --workspace 2>&1 | grep "test result:"
fi

# Summary
echo ""
echo "========================================"
echo "  Summary"
echo "========================================"
echo ""
echo "Run the following for detailed diagnostics:"
echo "  cargo run --example check_system -p miracast-doctor"
echo ""
echo "To test Wi-Fi P2P discovery:"
echo "  nmcli device wifi rescan"
echo "  nmcli device wifi list"
echo ""
echo "To test RTSP server:"
echo "  cargo run --example basic_server -p miracast-rtsp"
echo ""
echo "For full testing guide, see: docs/TESTING.md"
echo ""