# Integration Tests

These tests require real system services to be running.

## Prerequisites

- PipeWire session (usually auto-started with Wayland)
- GStreamer 1.x with x264enc plugin
- NetworkManager with P2P support
- xdg-desktop-portal-wlr

## Running Tests

```bash
# Check system requirements
just check-system

# Run integration tests
just test-integration-real

# Or manually:
cargo test --test integration_real -- --ignored
```

## Available Tests

| Test | Description | Requirements |
|------|-------------|--------------|
| test_pipewire_connection | Connect to PipeWire | PipeWire session |
| test_gstreamer_init | Initialize GStreamer | GStreamer |
| test_gstreamer_pipeline | Create and run pipeline | GStreamer + x264enc |
| test_networkmanager_dbus | Connect to NM via D-Bus | NetworkManager |
| test_portal_dbus | Connect to xdg-desktop-portal | xdg-desktop-portal |
| test_full_pipeline | End-to-end streaming | All services |
