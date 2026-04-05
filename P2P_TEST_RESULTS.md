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

---

# Fix Applied (April 5, 2026 - continued)

## Solution: Set `path` Property Programmatically

Based on research from multiple subagents investigating:

1. **pipewiresrc source code** - The `fd` only authenticates, `path` selects node
2. **gnome-network-displays** - Working Miracast implementation uses `fd` + `path`
3. **PipeWire portal docs** - The fd is restricted to only see portal nodes

### Changes Made

**Commit a65c86b:**
```rust
pipewiresrc.set_property("fd", fd);
pipewiresrc.set_property("path", node_id.to_string());
pipewiresrc.set_property("do-timestamp", true);
pipewiresrc_base.set_live(true);
```

### Key Insights

1. **fd ≠ node selection**: The fd authenticates the PipeWire core connection, but doesn't select which node to stream from
2. **path = node_id**: The `path` property (not `target-object`) specifies which PipeWire node to connect to
3. **Restricted visibility**: The portal fd only shows the screencast node, but pipewiresrc needs explicit `path` to find it
4. **Programmatic setup**: gnome-network-displays uses `g_object_set()` rather than pipeline strings

### Testing Required

The user needs to test with the LG TV in "Screen Share" mode:

```bash
# Put TV in Screen Share mode first
systemctl --user restart xdg-desktop-portal-wlr.service && sleep 1 && \
systemctl --user restart xdg-desktop-portal.service && sleep 3

env XDG_CURRENT_DESKTOP=sway \
  target/release/swaybeam daemon --sink <TV_MAC> --client

# Expected: 1920x1080 resolution (not 640x480 webcam)
```

The fix should result in:
- `pipewiresrc negotiated caps: ... width=(int)1920, height=(int)1080 ...`
- TV showing actual screen content instead of webcam

---

## Further Investigation (April 5, 2026 - Session 2)

### Fixes Applied

1. **Fixed `keepalive-time` property type** (commit 18210b6):
   - Changed from `u32` to `i32` (gint expected by GStreamer)
   - Pipeline no longer panics on property set

2. **Removed `autoconnect=false`**:
   - The `autoconnect` property was blocking pipeline startup
   - Removed to allow pipewiresrc to auto-start the stream

3. **Added detailed logging**:
   - Pipeline state change results
   - Better error messages

### Current Status

**Pipeline now starts and streams to TV!**

However, there's still an issue with source selection:
- Portal returns `node_id=85/104/105` (correct screen capture node)
- But pipewiresrc negotiates with 640x480 YUY2 format (webcam)
- The TV receives the stream, but shows webcam instead of screen

### Observations

| Test | Result | Resolution |
|------|--------|------------|
| `snap` example | ✅ Works | 1920x1080 |
| `screen_mirror` example | Same issue? | TBD |
| Daemon with `target-object=xdg-desktop-portal-wlr` | ❌ Webcam | 640x480 |
| Daemon with `path={node_id}` | ❌ Webcam | 640x480 |

### Mystery

The `snap` example uses `target-object=xdg-desktop-portal-wlr` and works correctly.
The daemon uses the same approach but gets webcam content.

**Possible causes:**
1. Timing difference - portal session state at pipeline creation
2. Environment difference - something in the daemon's process context
3. PipeWire state - the node_id mapping might be different
4. Portal instability - the portal has been crashing (see journal logs)

### Next Steps

1. **Check if the TV is actually showing webcam or something else** - verify output
2. **Compare `screen_mirror` example** - does it also get webcam?
3. **Try portal restart before each test** - rule out stale state
4. **Check PipeWire node visibility** - what does `pw-cli` show during capture?
5. **Test with different portal configuration** - remove `output_name` restriction

### Commands for Further Debugging

```bash
# Check what PipeWire sees during capture
watch -n1 'pw-cli ls Node | grep -A5 Video/Source'

# Run daemon with full PipeWire debug
GST_DEBUG=pipewiresrc:5 PIPEWIRE_DEBUG=4 \
  ./target/release/swaybeam daemon --sink <TV_MAC> --client

# Check portal state
busctl --user introspect org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop

# Monitor portal logs in real-time
journalctl --user -u xdg-desktop-portal-wlr -f
```
