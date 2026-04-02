# Architecture

## Overview

This project implements a Miracast source for wlroots-based compositors.

## Components

### doctor
Environment validation and system capability checks.

### capture
wlroots-based compositor screencast capture via xdg-desktop-portal-wlr and PipeWire.

### stream
Video/audio encoding and GStreamer pipeline management.

### net
Sink discovery, P2P group formation, Wi-Fi Direct via NetworkManager.

### rtsp
Miracast/WFD RTSP negotiation protocol implementation.

### daemon
Session orchestration and lifecycle management.

### cli
Command-line interface for operators.

## Dependencies

- wlroots-based compositor (Sway, River, Labwc, Hyprland, etc.)
- xdg-desktop-portal-wlr
- PipeWire
- GStreamer
- NetworkManager / wpa_supplicant
