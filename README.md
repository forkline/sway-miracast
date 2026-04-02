# sway-miracast

<p align="center">
  <strong>Wireless display streaming for Linux, the way it should be</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#getting-started">Getting Started</a> •
  <a href="#installation">Installation</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-alpha-orange" alt="Status: Alpha">
  <img src="https://img.shields.io/badge/rust-1.70+-blue" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License">
</p>

---

So you're running Sway (or another wlroots compositor) and want to cast your screen to a TV or projector without installing proprietary bloat or switching to GNOME? Same here.

sway-miracast is a Miracast source implementation built from the ground up for wlroots-based Wayland compositors. It talks Wi-Fi Direct, handles the WFD protocol negotiation, captures your screen via PipeWire, and streams H.264 video to any Miracast-compatible receiver.

## What This Does

- **Discovers** Miracast receivers via Wi-Fi Direct P2P
- **Negotiates** capabilities using the WFD RTSP protocol
- **Captures** your screen through xdg-desktop-portal-wlr
- **Encodes** to H.264 and transmits over RTP

No proprietary drivers. No vendor lock-in. Just protocols that should've worked out of the box years ago.

## Features

- Native Wayland support via xdg-desktop-portal-wlr
- Wi-Fi Direct P2P networking through NetworkManager
- H.264 hardware encoding via GStreamer
- Modular architecture (each crate does one thing well)
- Real-time capability negotiation
- Comprehensive system diagnostics

## Getting Started

### Prerequisites

You'll need:
- Sway or another wlroots-based compositor
- xdg-desktop-portal-wlr configured for screen capture
- PipeWire with screen sharing support
- NetworkManager with P2P/Wi-Fi Direct support
- A Wi-Fi adapter that speaks Wi-Fi Direct (not all do)
- GStreamer with H.264 codecs (openh264 or x264)

### Quick Check

Run the diagnostics to see if your system is ready:

```bash
cargo run --bin doctor
```

This checks all dependencies and tells you what's missing.

### Installation

**Debian/Ubuntu:**
```bash
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
                 gstreamer1.0-libav gstreamer1.0-tools \
                 pipewire libspa-0.2-bluetooth network-manager
```

**Fedora/RHEL:**
```bash
sudo dnf install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
                 gstreamer1.0-libav pipewire NetworkManager-wifi
```

**Build:**
```bash
git clone https://github.com/forkline/sway-miracast.git
cd sway-miracast
cargo build --release
```

## Usage

*Note: The CLI is still under development. The following shows the intended API.*

**Scan for receivers:**
```bash
miracast scan
```

**Connect and stream:**
```bash
miracast connect "Living Room TV"
miracast stream
```

**Check system health:**
```bash
miracast doctor
```

## Architecture

The project is split into focused crates:

| Crate | Purpose |
|-------|---------|
| `miracast-doctor` | System capability validation |
| `miracast-net` | Wi-Fi Direct P2P networking |
| `miracast-rtsp` | WFD protocol negotiation |
| `miracast-capture` | Screen capture via PipeWire |
| `miracast-stream` | GStreamer encoding pipeline |
| `miracast-daemon` | Session orchestration |
| `miracast-cli` | Command-line interface |

Each crate is independently testable and has a clear API boundary.

## Roadmap

- [x] Project foundation
- [x] System diagnostics (doctor)
- [ ] P2P networking (in progress)
- [ ] RTSP/WFD negotiation (in progress)
- [ ] Screen capture
- [ ] Video encoding
- [ ] Full integration

## Contributing

Found a bug? Missing a feature? Hardware compatibility issues?

1. Check existing issues
2. Run `miracast doctor` and include the output
3. Open a PR with tests for new functionality

Code should:
- Follow Rust naming conventions
- Include tests for public APIs
- Use `thiserror` for error types
- Document public items with `///` comments

## Known Limitations

- Wi-Fi Direct hardware support varies wildly between adapters
- Performance depends heavily on wireless conditions
- Currently alpha quality - expect rough edges

## Development

```bash
# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-targets

# System verification
./scripts/test-system.sh
```

## License

MIT. Use it, modify it, share it.

---

*Built with frustration by people who just wanted their screens to work.*
