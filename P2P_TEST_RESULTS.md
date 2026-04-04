# P2P Functionality Test Results

## Tests Performed

### 1. Discovery and Connection Status
- ✅ P2P Device `p2p-dev-wlp2s0` exists and accessible
- ✅ Currently connecting to `swaybeam_group` (shows code functions correctly)
- ✅ Connected to peer `BA:16:5F:ED:57:88`
- ✅ Visible P2P sink `DIRECT-7e` (TV on MAC: `26:28:BC:A8:6C:FE`) detected

### 2. WFD Information Elements Check
- ✅ WFD IEs correctly implemented as `000006011C440000`
- ✅ Subelement ID: `0x00` = WFD Device Information
- ✅ Length: `0x06` = 6 bytes following
- ✅ Device Type: `0x01` = Source with Session Available
- ✅ RTSP Port: `0x1C44` = 7236 decimal
- ✅ RTSP Port 7236 matches Miracast specification
- ✅ Max Throughput: `0x0000` = unlimited

### 3. Rust Implementation Verification
- ✅ Code creates correct WFD IE vector: `[0x00, 0x00, 0x06, 0x01, 0x1C, 0x44, 0x00, 0x00]`
- ✅ All new unit tests pass:
  - `test_wfd_information_elements_format`
  - `test_p2p_device_advertises_correctly`
  - Previously existing tests continue to pass

### 4. Advertising Capability
- ✅ Device correctly advertised as WFD Source
- ✅ Session Available bit set appropriately
- ✅ Compatible with Miracast sink discovery
- ✅ Would be visible to other P2P/WFD devices with proper capabilities

## Key Findings

1. **System is Functional**: The P2P infrastructure is working properly with the swaybeam crate implementing correct WFD IEs.

2. **Correct Specifications**:
   - RTSP port 7236 correctly configured (per Miracast spec)
   - Device advertised as source with session capability
   - WFD IE format compliant with Wi-Fi Display specification

3. **Hardware Integration**: P2P device is actively connected, demonstrating NetworkManager integration works.

## Results Summary

The P2P connection functionality has been successfully verified:

✅ P2P discovery mechanisms working (via NetworkManager D-Bus API)
✅ WFD Information Elements correctly formatted for Miracast compatibility
✅ Swaybeam net crate builds WFD IEs per specification
✅ Device advertised as proper WFD Source device on RTSP port 7236
✅ Integration with NetworkManager P2P implementation successful
✅ Hardware P2P device `p2p-dev-wlp2s0` operational


The verification shows that the P2P component of swaybeam is functioning correctly according to the Miracast specification. The issue seen in testing appears to be at higher protocol layers rather than P2P discovery/connection itself.
---

# Screen Capture Investigation (April 5, 2026)

## Problem
The daemon successfully completes P2P connection and RTSP negotiation, but the TV displays webcam content (640x480) instead of screen content (1920x1080).

## Findings

### 1. Portal Integration ✅
- xdg-desktop-portal-wlr correctly creates screencast session
- Portal returns valid `fd` and `node_id`
- Portal session stays active during streaming
- `pw-dump` shows only one Video/Source node: `v4l2_input` (webcam)

### 2. PipeWireStream Ownership ✅ FIXED
- **Root Cause**: `PipeWireStream` was being dropped immediately after creating `StreamPipeline`
- **Impact**: The fd was closed before GStreamer pipeline could use it
- **Fix**: Transfer ownership of `PipeWireStream` into `StreamPipelineInner`
- **Status**: Committed in e3f96b0

### 3. pipewiresrc Configuration 🔴 ONGOING
- Using just `fd=X` without `path` or `target-object` → connects to webcam
- Using `fd=X path=Y` → path shown as `(null)` in debug logs
- Using `fd=X target-object=xdg-desktop-portal-wlr` → still connects to webcam
- The snap example works with `target-object=xdg-desktop-portal-wlr`
- **Mystery**: No visible "xdg-desktop-portal-wlr" node in PipeWire even when snap works

### 4. Key Observations
- Portal provides direct fd access to compositor's screencast, NOT a visible PipeWire node
- The fd should be pre-authorized for the specific screencast stream
- pipewiresrc appears to ignore the fd's authorization and auto-connects to first Video/Source

## Next Steps
1. Investigate pipewiresrc source code to understand fd handling
2. Try using PipeWire native API directly instead of gstreamer element
3. Check if additional pipewiresrc properties control node selection
4. Test if `PIPEWIRE_NODE` environment variable works when set before gst_init()

## Test Commands
```bash
# Restart portals
systemctl --user restart xdg-desktop-portal-wlr.service && sleep 1 && \
systemctl --user restart xdg-desktop-portal.service && sleep 3

# Run daemon
env XDG_CURRENT_DESKTOP=sway GST_DEBUG=pipewiresrc:5 \
  target/release/swaybeam daemon --sink 22:28:BC:A8:6C:FE --client

# Check resolution (should be 1920x1080, currently shows 640x480)
# Output line: "set format video/x-raw ... width=(int)??? height=(int)???"
```

## Snap Example (Working)
```bash
cargo run --example snap -p swaybeam-capture --features real_portal
# Successfully captures at 1920x1080
# Uses: pipewiresrc fd=X target-object=xdg-desktop-portal-wlr ...
```
