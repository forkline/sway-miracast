# sway-miracast

sway-miracast is a Miracast implementation designed specifically for Sway and other wlroots-based Wayland compositors. It enables wireless display streaming from Linux systems to Miracast-compatible TVs, monitors, and projectors using Wi-Fi Direct.

## Project Overview

The project implements the complete Miracast protocol stack:
- Wi-Fi Direct device discovery and P2P connection establishment
- WFD (Wi-Fi Display) RTSP-based capability negotiation
- Screen capture from Sway/wlroots via xdg-desktop-portal-wlr
- GStreamer-based H.264 video encoding for wireless streaming
- Real-time RTP transmission to target displays

Target users include anyone seeking to extend their Linux desktop wirelessly to larger screens without proprietary software.

## Quick Start Guide

Prerequisites:
- Sway or wlroots-based Wayland compositor with xdg-desktop-portal-wlr
- PipeWire running with screen sharing support
- GStreamer with H.264 encoding codecs
- NetworkManager with P2P support
- Wi-Fi adapter supporting Wi-Fi Direct

Basic usage (coming soon once CLI is implemented):
```bash
# Diagnose system capabilities (currently working)
cargo run --bin doctor

# Discover compatible displays (will work when complete)
cargo run --bin cli scan

# Connect to display (will work when complete)
cargo run --bin cli connect [DISPLAY_NAME]

# Start streaming (will work when complete)
cargo run --bin cli stream
```

## Installation

### Building from Source:

Clone the repository:
```bash
git clone https://github.com/yourname/sway-miracast.git
cd sway-miracast
```

Ensure dependencies are installed:
- Rust 1.70+ (with stable toolchain)
- NetworkManager
- GStreamer 1.0 with plugins (openh264, x264, h264parse, rtph264pay)
- PipeWire or compatibility layer
- xdg-desktop-portal-wlr

Build the project:
```bash
cargo build --release
```

### Dependencies

Required system packages (Ubuntu/Debian):
```bash
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
                 gstreamer1.0-libav gstreamer1.0-tools \
                 pipewire pipewire-audio libspa-0.2-bluetooth \
                 network-manager network-manager-openvpn-gnome
```

Required system packages (Fedora/RHEL):
```bash
sudo dnf install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
                 gstreamer1.0-libav gstreamer1.0-devel \
                 pipewire pipewire-devel NetworkManager-wifi \
                 openh264-plugin
```

## Usage Examples

Once complete, the following commands will be available:

Discover and show available Miracast displays:
```bash
# Basic discovery
cargo run --bin cli scan

# With detailed information
cargo run --bin cli scan --verbose
```

System health check:
```bash
# Check all dependencies
cargo run --bin doctor

# Check specific component
cargo run --bin doctor --check pipewire
```

Start wireless display streaming:
```bash
# Stream at specific resolution
cargo run --bin cli stream --target TV_NAME --resolution 1080x720

# Stop current session
cargo run --bin cli stop
```

## Architecture Summary

The system is organized as follows:

- **doctor**: Validates all system dependencies for Miracast capability
- **net**: Wi-Fi Direct P2P networking using NetworkManager
- **rtsp**: WFD protocol state machine and negotiation
- **capture**: Screen capture via xdg-desktop-portal-wlr and PipeWire
- **stream**: GStreamer video encoding pipeline and RTP packetization
- **daemon**: Orchestration and session management
- **cli**: Command-line interface (currently being developed)

The architecture emphasizes modularity, with clean API boundaries between components and extensive testing support.

## Contributing

We welcome contributions of all kinds:

1. Bug reports: Please provide `sway-miracast doctor` output along with error details
2. Pull requests: Focus on one logical change, include tests for new functionality
3. Feature suggestions: Open issues for discussion of new capabilities
4. Testing: Report compatibility with various Miracast receivers
5. Documentation: Improve this README and internal comments

To contribute code:
```bash
# Fork the repository and clone your fork
git clone https://github.com/YOUR_USERNAME/sway-miracast.git
cd sway-miracast

# Create a feature branch
git checkout -b my-feature-branch

# Make changes and run tests
cargo test

# Submit a pull request
```

All code follows the Rust style guidelines and includes comprehensive unit tests.

## Development Status

**Status: Alpha** - Components under active development, API subject to change

Current status by milestone ([view roadmap](docs/milestones.md)):
- ✅ Project foundation and workspace setup
- ✅ Basic doctor and system validation tools 
- 🔄 P2P networking in progress (`net` crate)
- 🔄 WFD protocol negotiation implemented (`rtsp` crate)
- 🚧 Screen capture from Wayland (planned: `capture` crate)
- 🚧 Video encoding pipeline (planned: `stream` crate)
- ⏳ Complete integration and testing (planned)

### Limitations

- Requires Wayland compositor supporting xdg-desktop-portal-wlr (Sway/wlroots primarily)
- Wi-Fi Direct capability needed on host system
- Early implementation may have reliability or performance issues
- Hardware acceleration support still under development

## Security

The implementation follows security best practices:
- Network communications run over secure P2P links
- Local privilege elevation follows principle of least privilege
- User consent required for screen sharing (via portal interfaces)
- Input validation on protocol parameters prevents injection
- Cryptographic functions provided by platform libraries only

Note that screen content will transmit over wireless networks - consider sensitivity of displayed materials.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

Copyright (c) 2026 sway-miracast contributors.