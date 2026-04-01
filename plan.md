# Implementation Plan: Miracast Source for Sway / wlroots

## Goal

Build a Linux Miracast **source** implementation that works on **Sway/wlroots**, can stream to an **LG webOS TV**, and is structured so that:

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

1. Use **Sway/wlroots capture** through **xdg-desktop-portal-wlr + PipeWire**
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
  - Sway/wlroots screencast capture
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
miracast-sway/
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
