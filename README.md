# sway-miracast

<p align="center">
  <strong>Miracast source implementation for Sway and wlroots-based compositors</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-alpha-orange" alt="Status: Alpha">
  <img src="https://img.shields.io/badge/rust-2021-blue" alt="Rust 2021 edition">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
</p>

`sway-miracast` is an open-source project focused on bringing Miracast support to Sway and other wlroots-based Wayland desktops.
It is built as a modular Rust workspace covering discovery, negotiation, capture, streaming, and session orchestration.

The goal is straightforward: make it possible to cast a wlroots desktop to a Miracast receiver with tools that fit naturally into a Linux setup.

## Current Status

This project is in alpha.

Today, the repository provides the workspace structure, system diagnostics, and early implementations of the network, RTSP, capture, streaming, daemon, and CLI crates. The full end-to-end Miracast session is still under active development.

If you are looking for a finished daily-driver tool, it is not there yet. If you want to follow the work, test hardware, or help push it forward, you are in the right place.

## Scope

The project is intended to cover the full Miracast source side:

- Wi-Fi Direct discovery and connection setup
- WFD capability negotiation over RTSP
- Screen capture through `xdg-desktop-portal-wlr` and PipeWire
- H.264 encoding and RTP transport via GStreamer
- Session orchestration and a user-facing CLI

## Workspace

| Crate | Role |
| --- | --- |
| `miracast-doctor` | Checks whether the host system is capable of running a Miracast session |
| `miracast-net` | Handles Wi-Fi Direct and P2P networking |
| `miracast-rtsp` | Implements WFD RTSP negotiation |
| `miracast-capture` | Integrates screen capture through portal and PipeWire APIs |
| `miracast-stream` | Builds the GStreamer video pipeline |
| `miracast-daemon` | Coordinates the full session lifecycle |
| `miracast-cli` | Exposes user-facing commands |

## Requirements

To be useful on a real machine, the project expects:

- Sway or another wlroots-based compositor
- `xdg-desktop-portal-wlr`
- PipeWire with screen sharing support
- NetworkManager with Wi-Fi Direct support
- A Wi-Fi adapter with workable P2P support
- GStreamer with H.264-related plugins

Hardware support for Miracast on Linux is uneven, especially around Wi-Fi Direct. The `doctor` crate exists partly to make that visible early.

## Installation

Arch Linux is the primary target for this project and the first place new setup instructions should work well.

### Arch Linux

Install the core runtime dependencies first:

```bash
sudo pacman -S --needed \
  gstreamer \
  gst-plugins-base \
  gst-plugins-good \
  gst-plugins-bad \
  gst-libav \
  pipewire \
  wireplumber \
  networkmanager \
  wpa_supplicant \
  xdg-desktop-portal \
  xdg-desktop-portal-wlr
```

Then build the project:

```bash
git clone https://github.com/forkline/sway-miracast.git
cd sway-miracast
cargo build
```

### Debian / Ubuntu

Package names differ a bit, but the rough equivalent set is:

```bash
sudo apt install \
  gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad \
  gstreamer1.0-libav \
  gstreamer1.0-tools \
  pipewire \
  network-manager \
  xdg-desktop-portal \
  xdg-desktop-portal-wlr
```

### Fedora

```bash
sudo dnf install \
  gstreamer1-plugins-base \
  gstreamer1-plugins-good \
  gstreamer1-plugins-bad-free \
  gstreamer1-libav \
  pipewire \
  NetworkManager \
  xdg-desktop-portal \
  xdg-desktop-portal-wlr
```

## Getting Started

Clone and build the workspace:

```bash
git clone https://github.com/forkline/sway-miracast.git
cd sway-miracast
cargo build
```

Run the system checks:

```bash
cargo run --bin doctor
```

Run the test suite:

```bash
cargo test --workspace
```

## Development

Common commands:

```bash
just test
just lint
just pre-commit
```

If `just` is not installed, the underlying cargo commands also work:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

## Roadmap

- [x] Workspace and crate layout
- [x] Initial system diagnostics
- [ ] Harden Wi-Fi Direct discovery and connection flow
- [ ] Complete WFD RTSP negotiation
- [ ] Wire capture and streaming into a real session
- [ ] Stabilize the CLI and daemon flow
- [ ] Test against a broader set of receivers

## Contributing

Contributions are welcome, especially in these areas:

- Wi-Fi Direct compatibility across adapters and chipsets
- WFD protocol correctness
- PipeWire and portal integration
- Receiver interoperability testing
- Documentation and developer ergonomics

Even small contributions are useful here. A test report from unusual hardware, a cleaner error message, or a note about distro-specific setup can save someone else a lot of time.

If you open an issue for a runtime problem, include:

- your compositor and distro
- Wi-Fi hardware details
- output from `cargo run --bin doctor`
- logs or reproduction steps

## License

MIT. See [`LICENSE`](LICENSE).
