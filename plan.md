# Implementation Plan: Miracast Source for wlroots-based compositors

## Goal

Build a Linux Miracast **source** implementation that works on **wlroots-based compositors** (Sway, River, Labwc, Hyprland), can stream to **Miracast-compatible TVs**, and is structured so that:

- capture is compositor-friendly
- Miracast networking/control is isolated
- RTSP/WFD negotiation is testable independently
- the system can later support both:
  - **mirror mode**
  - **virtual second-monitor mode**

---

## Non-Goals for v1

- HDCP
- UIBC / remote keyboard-mouse over Miracast
- perfect interoperability with every sink
- KDE/GNOME-specific integration
- polished GUI

---

## High-Level Strategy

Do **not** build on MiracleCast source mode as the main foundation.

Instead:

1. Use **wlroots compositor capture** through **xdg-desktop-portal-wlr + PipeWire**
2. Use **GStreamer** for encode + transport plumbing
3. Use **NetworkManager + wpa_supplicant** for Wi-Fi Direct / P2P orchestration
4. Implement a new **Rust Miracast RTSP/WFD source stack**
5. Keep all layers modular and separately testable

---

## Architecture

### Main components

- `doctor`
  - environment validation
  - system capability checks
- `capture`
  - wlroots compositor screencast capture
  - PipeWire integration
- `stream`
  - video/audio encode
  - media pipeline lifecycle
- `net`
  - sink discovery
  - P2P group formation
  - IP readiness
- `rtsp`
  - Miracast/WFD RTSP negotiation
  - capability/state machine
- `daemon`
  - orchestration
  - session lifecycle
- `cli`
  - operator-facing commands

---

## Repository Layout

```text
swaybeam/
  Cargo.toml
  crates/
    doctor/
    capture/
    stream/
    net/
    rtsp/
    daemon/
    cli/
  docs/
    architecture.md
    protocol.md
    test-matrix.md
    milestones.md
  scripts/
    dev-env.sh
    run-dummy-sink.sh
