# sway-miracast

**Miracast source for Sway/wlroots compositors**

Stream your screen wirelessly to Miracast-compatible TVs and displays.

## Quick Start

```bash
# 1. Clone and build
git clone https://github.com/forkline/sway-miracast.git
cd sway-miracast
cargo build --release

# 2. Check if your system is ready
./target/release/miracast doctor

# 3. Run the daemon (when all checks pass)
./target/release/miracast daemon
```

## Requirements

| Component | Why Needed | Install (Arch) |
|-----------|-----------|----------------|
| Sway/River/Labwc | Wayland compositor | `sway` |
| WiFi adapter with P2P | Wi-Fi Direct for Miracast | Hardware |
| PipeWire | Audio/video handling | `pipewire wireplumber` |
| GStreamer | H.264/H.265 encoding | `gst-plugins-ugly` |
| NetworkManager | P2P connection management | `networkmanager` |
| xdg-desktop-portal-wlr | Screen capture | `xdg-desktop-portal-wlr` |

## Installation

### Arch Linux (Recommended)

```bash
# Install dependencies
sudo pacman -S --needed \
    rust gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly \
    pipewire wireplumber networkmanager wpa_supplicant \
    xdg-desktop-portal xdg-desktop-portal-wlr

# Build
git clone https://github.com/forkline/sway-miracast.git
cd sway-miracast
cargo build --release

# Install (optional)
sudo cp target/release/miracast /usr/local/bin/
```

### Ubuntu/Debian

```bash
sudo apt install \
    rustc cargo gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-libav \
    pipewire wireplumber network-manager wpa_supplicant \
    xdg-desktop-portal-wlr

cargo build --release
```

## Usage

### Check System Readiness

```bash
miracast doctor
```

Expected output when ready:
```
✓ Sway Compositor: Running under Sway
✓ PipeWire: PipeWire daemon and session manager running
✓ GStreamer: H.264 ready, H.265/4K ready
✓ NetworkManager: NetworkManager daemon running
✓ WPA Supplicant: wpa_supplicant daemon running
✓ XDG Desktop Portal: xdg-desktop-portal running
```

### Discover Miracast Displays

```bash
miracast discover --timeout 10
```

### Connect to a Display

```bash
miracast connect --sink "Living Room TV"
```

### Start Streaming

```bash
# 1080p (default)
miracast stream

# 4K at 30fps
miracast stream --width 3840 --height 2160 --framerate 30

# 4K at 60fps
miracast stream --width 3840 --height 2160 --framerate 60
```

### Disconnect

```bash
miracast disconnect
```

### Run Full Daemon

```bash
miracast daemon
```

The daemon handles the full Miracast session automatically:
1. Checks system requirements
2. Discovers available sinks
3. Connects via Wi-Fi Direct P2P
4. Negotiates capabilities via RTSP
5. Starts screen capture and streaming
6. Handles disconnection gracefully

## CLI Commands

```
miracast doctor              # Check system requirements
miracast discover [-t N]      # Discover Miracast displays
miracast connect -s <name>   # Connect to a display
miracast stream [options]    # Start streaming
miracast disconnect          # Disconnect from display
miracast daemon              # Run full session
miracast status              # Show connection status
```

## Video Codecs

| Codec | Resolution | Bitrate | Use Case |
|-------|------------|---------|----------|
| H.264 | 1080p | 8 Mbps | Universal compatibility |
| H.265 | 4K | 20 Mbps | Better quality for 4K TVs |
| AV1 | Any | 5 Mbps | Future-proof, best compression |

H.265 is automatically used for 4K streaming. H.264 is used for 1080p for maximum compatibility.

## Development

```bash
# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt

# Run with debug logging
RUST_LOG=debug cargo run -- daemon
```

## Troubleshooting

### "No WiFi hardware detected"
Install a WiFi adapter that supports P2P (Wi-Fi Direct). Most modern USB adapters work.

### "Not running a Wayland compositor"
Miracast requires a Wayland compositor like Sway. Run under Sway, River, Labwc, or Hyprland.

### "Missing H.264 plugins"
Install GStreamer plugins:
```bash
# Arch
sudo pacman -S gst-plugins-ugly

# Ubuntu
sudo apt install gstreamer1.0-plugins-ugly
```

### Portal not responding
Ensure portal services are running:
```bash
systemctl --user start xdg-desktop-portal.service
systemctl --user start xdg-desktop-portal-wlr.service
```

## Architecture

```
┌─────────────────────────────────────────────┐
│                  CLI (miracast)              │
└──────────────────────┬──────────────────────┘
                       │
┌──────────────────────▼──────────────────────┐
│                 Daemon                       │
│  Orchestrates: discover, connect, stream    │
└──────┬───────┬───────┬───────┬───────┬──────┘
       │       │       │       │       │
    Doctor  Capture  Stream   Net   RTSP
    (check) (screen) (encode) (P2P) (WFD)
```

## Status

- ✅ System diagnostics (doctor)
- ✅ Wi-Fi Direct discovery (net)
- ✅ RTSP/WFD negotiation (rtsp)
- ✅ Screen capture via portal (capture)
- ✅ GStreamer H.264/H.265/AV1 encoding (stream)
- ✅ Session orchestration (daemon)
- ✅ CLI interface
- ⏳ Real hardware testing needed

## License

MIT
