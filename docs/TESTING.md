# Testing Guide

This document describes how to test the sway-miracast project on your laptop.

## Prerequisites

### Hardware Requirements

1. **Wi-Fi Adapter with P2P/Wi-Fi Direct support**
   - Most modern Intel, Realtek, and Atheros cards support this
   - Check with: `iw list | grep -A 10 "Supported interface" | grep P2P`
   - Example output should show: `* P2P-client`, `* P2P-GO`, `* P2P-device`

2. **Miracast-compatible receiver** (for end-to-end testing)
   - Smart TV with Miracast support (LG webOS, Samsung, Sony, etc.)
   - Microsoft Wireless Display Adapter
   - Roku streaming device
   - Amazon Fire TV Stick
   - Or any Miracast-certified display adapter

3. **Linux laptop with Sway/wlroots**
   - Running Sway compositor (not GNOME/KDE for capture)
   - Wayland session active

### Software Requirements

Install required packages:

**Ubuntu/Debian:**
```bash
sudo apt install -y \
    build-essential rustc cargo \
    libpipewire-0.3-dev \
    gstreamer1.0-tools \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    network-manager \
    wpasupplicant \
    xdg-desktop-portal-wlr
```

**Fedora/RHEL:**
```bash
sudo dnf install -y \
    rust cargo gcc \
    pipewire-devel \
    gstreamer1-devel \
    gstreamer1-plugins-base \
    gstreamer1-plugins-good \
    gstreamer1-plugins-bad-free \
    gstreamer1-plugins-ugly-free \
    NetworkManager-wifi \
    wpa_supplicant \
    xdg-desktop-portal-wlr
```

**Arch Linux:**
```bash
sudo pacman -S --needed \
    rust base-devel \
    pipewire \
    gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav \
    networkmanager \
    wpa_supplicant \
    xdg-desktop-portal-wlr
```

## Quick Start Testing

### 1. Build the Project

```bash
cd sway-miracast
cargo build --release
```

### 2. Run Unit Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p miracast-doctor
cargo test -p miracast-capture
cargo test -p miracast-stream
cargo test -p miracast-net
cargo test -p miracast-rtsp

# Run tests with verbose output
cargo test --workspace -- --nocapture
```

### 3. Run System Diagnostics

The doctor crate checks if your system is ready for Miracast:

```bash
# Run the doctor example
cargo run --example check_system -p miracast-doctor

# Or build and run the binary
cargo build --release
./target/release/examples/check_system
```

Expected output shows status of each component:
```
Miracast Doctor - Environment Check Report
==========================================
✓ Sway Compositor: Running under Sway compositor
✓ PipeWire: PipeWire daemon and session manager running
✓ GStreamer: GStreamer and required H.264 encoding plugins found
✓ NetworkManager: NetworkManager daemon running
✓ WPA Supplicant: wpa_supplicant daemon running
✓ XDG Desktop Portal: xdg-desktop-portal with WLR backend running

✓ All checks passed! Your system is ready for Miracast.
```

### 4. Test Network Discovery

Test P2P device discovery (requires Wi-Fi adapter):

```bash
# Run the net example
cargo run --example discover_and_connect -p miracast-net

# Or use nmcli directly to test
nmcli device wifi list
nmcli device wifi rescan
```

### 5. Test RTSP Server

Test the RTSP/WFD protocol implementation:

```bash
# Run the RTSP example
cargo run --example basic_server -p miracast-rtsp

# In another terminal, test with a simple RTSP client
# Install rtsp-client-simple or use netcat:
echo -e "OPTIONS rtsp://localhost:7236 RTSP/1.0\r\nCSeq: 1\r\n\r\n" | nc localhost 7236
```

## Component-Specific Testing

### Testing Doctor Crate

The doctor crate has system checks that can be run individually:

```bash
# Check if Sway is running
cargo test -p miracast-doctor test_check_all_returns_report -- --nocapture

# The check_all() function tests:
# - Sway compositor presence
# - PipeWire daemon and session manager
# - GStreamer plugins (openh264, x264, h264parse, rtph264pay)
# - NetworkManager availability
# - wpa_supplicant for P2P
# - xdg-desktop-portal-wlr backend
```

### Testing Capture Crate

The capture crate validates screen capture configuration:

```bash
# Run capture tests
cargo test -p miracast-capture -- --nocapture

# Tests verify:
# - Configuration validation (width, height, framerate)
# - Error handling for invalid configs
# - Default values
```

### Testing Stream Crate

The stream crate tests video/audio pipeline configuration:

```bash
# Run stream tests
cargo test -p miracast-stream -- --nocapture

# Tests verify:
# - Video codec (H264) configuration
# - Audio codec (AAC, LPCM) configuration
# - Pipeline input/output setup
# - Error handling
```

### Testing Net Crate

The net crate tests Wi-Fi Direct P2P functionality:

```bash
# Run net tests
cargo test -p miracast-net -- --nocapture

# For real network testing, you need:
# - Wi-Fi adapter with P2P support
# - NetworkManager running
# - Active Wi-Fi interface
```

### Testing RTSP Crate

The RTSP crate tests WFD protocol negotiation:

```bash
# Run RTSP tests
cargo test -p miracast-rtsp -- --nocapture

# Tests verify:
# - WFD capability negotiation
# - Session state machine
# - RTSP message parsing
# - Server functionality
```

## Integration Testing

### Test with Mock Components

Run integration tests that simulate the full workflow:

```bash
# Run integration tests
cargo test --workspace --test '*'

# Specific integration test files:
cargo test --test integration_doctor_net
cargo test --test integration_daemon_flow
cargo test --test integration_rtsp_stream
```

### Test with Real Miracast Device

1. **Enable Wi-Fi Direct on your TV/receiver**
   - Usually in Settings > Network > Screen Mirroring
   - Put it in "listening" or "discoverable" mode

2. **Discover devices**
   ```bash
   # Use nmcli to scan for P2P devices
   nmcli device wifi rescan
   nmcli device wifi list
   
   # Or use the net crate example
   cargo run --example discover_and_connect -p miracast-net
   ```

3. **Connect to the device**
   ```bash
   # Using nmcli
   nmcli device wifi connect <DEVICE_NAME>
   
   # Check assigned IP
   ip addr show
   ```

4. **Start RTSP negotiation**
   ```bash
   # The RTSP server should handle WFD negotiation
   cargo run --example basic_server -p miracast-rtsp
   ```

## Manual Testing Procedures

### Test 1: Verify System Readiness

```bash
# Check all prerequisites
./target/release/examples/check_system

# Verify individual components:
swaymsg -t get_version          # Sway running?
systemctl --user status pipewire # PipeWire active?
gst-inspect-1.0 x264             # H.264 encoder available?
nmcli general status             # NetworkManager running?
systemctl status wpa_supplicant  # P2P support ready?
```

### Test 2: Verify Wi-Fi P2P Capability

```bash
# Check Wi-Fi adapter capabilities
iw list | grep -A 20 "Supported interface" | grep P2P

# Check if P2P device exists
iw dev | grep -A 5 "Interface"

# Test P2P discovery
wpa_cli p2p_find
# Wait 10 seconds
wpa_cli p2p_peers
wpa_cli p2p_stop_find
```

### Test 3: Test RTSP Server

```bash
# Start the RTSP server
cargo run --example basic_server -p miracast-rtsp &

# Test with telnet/netcat
telnet 127.0.0.1 7236

# Send OPTIONS request
OPTIONS rtsp://localhost:7236 RTSP/1.0
CSeq: 1

# Press Enter twice after the blank line

# Expected response:
# RTSP/1.0 200 OK
# CSeq: 1
# Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN
```

### Test 4: Test GStreamer Pipeline

```bash
# Verify GStreamer plugins
gst-inspect-1.0 | grep -E "x264|h264|rtp"

# Test simple H.264 encoding
gst-launch-1.0 -v videotestsrc num-buffers=100 ! \
    videoconvert ! x264enc ! h264parse ! \
    rtph264pay ! udpsink host=127.0.0.1 port=5000

# In another terminal, receive the stream:
gst-launch-1.0 udpsrc port=5000 ! \
    application/x-rtp, media=video, encoding-name=H264 ! \
    rtph264depay ! h264parse ! avdec_h264 ! \
    videoconvert ! autovideosink
```

## Troubleshooting

### Common Issues

1. **"Sway not detected"**
   ```bash
   # Ensure SWAYSOCK is set
   echo $SWAYSOCK
   
   # If empty, source sway env
   export SWAYSOCK=$(sway --get-socketpath)
   ```

2. **"PipeWire not running"**
   ```bash
   # Start PipeWire
   systemctl --user start pipewire pipewire-pulse
   
   # Or manually
   pipewire &
   pipewire-pulse &
   ```

3. **"GStreamer plugins missing"**
   ```bash
   # Install missing plugins
   sudo apt install gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
   
   # Verify
   gst-inspect-1.0 x264
   ```

4. **"Wi-Fi P2P not supported"**
   ```bash
   # Check your Wi-Fi adapter
   iw list | grep -i p2p
   
   # If no output, your adapter may not support P2P
   # Consider getting a USB Wi-Fi adapter with P2P support
   # Recommended: Intel AX200/AX210, Realtek RTL8812AU
   ```

5. **"xdg-desktop-portal-wlr not running"**
   ```bash
   # Start the portal
   systemctl --user start xdg-desktop-portal-wlr
   
   # Or manually
   /usr/lib/xdg-desktop-portal-wlr &
   ```

6. **"NetworkManager not managing Wi-Fi"**
   ```bash
   # Check NetworkManager status
   systemctl status NetworkManager
   
   # Enable if disabled
   sudo systemctl enable --now NetworkManager
   
   # Ensure Wi-Fi is managed
   nmcli device status
   ```

### Debug Mode

Run with debug logging:

```bash
RUST_LOG=debug cargo run --example check_system -p miracast-doctor
RUST_LOG=trace cargo test --workspace -- --nocapture
```

### Test Logs

Check test execution logs:

```bash
# Run tests with output capture disabled
cargo test --workspace -- --nocapture --test-threads=1

# Save test output to file
cargo test --workspace 2>&1 | tee test_output.log
```

## Testing Checklist

Before reporting issues, verify:

- [ ] Rust toolchain is up to date (`rustc --version`)
- [ ] All dependencies installed
- [ ] Sway compositor running
- [ ] PipeWire daemon active
- [ ] GStreamer plugins available
- [ ] NetworkManager running
- [ ] Wi-Fi adapter supports P2P
- [ ] wpa_supplicant available
- [ ] xdg-desktop-portal-wlr running
- [ ] All unit tests pass (`cargo test --workspace`)

## Reporting Test Results

When reporting issues, include:

1. System information:
   ```bash
   uname -a
   rustc --version
   cargo --version
   ```

2. Doctor output:
   ```bash
   cargo run --example check_system -p miracast-doctor > doctor_output.txt
   ```

3. Test results:
   ```bash
   cargo test --workspace > test_results.txt 2>&1
   ```

4. System logs:
   ```bash
   journalctl --user -u pipewire > pipewire.log
   journalctl -u NetworkManager > networkmanager.log
   ```

## Next Steps

Once all tests pass:

1. Try discovering a real Miracast device
2. Test RTSP negotiation with the device
3. Test screen capture (requires xdg-desktop-portal integration)
4. Test full streaming pipeline

For development, see [architecture.md](architecture.md) for component details.