# Miracast Real-World Test Scenarios

This document provides comprehensive test scenarios and procedures for testing swaybeam with real Miracast-compatible displays (TVs, monitors, projectors).

## Table of Contents

1. [Test Environment Setup](#test-environment-setup)
2. [Test Scenarios](#test-scenarios)
   - [Scenario 1: Initial Discovery and Connection](#scenario-1-initial-discovery-and-connection)
   - [Scenario 2: Capability Negotiation](#scenario-2-capability-negotiation)
   - [Scenario 3: Codec Negotiation](#scenario-3-codec-negotiation)
   - [Scenario 4: Resolution/Framerate Negotiation](#scenario-4-resolutionframerate-negotiation)
   - [Scenario 5: Session Establishment](#scenario-5-session-establishment)
   - [Scenario 6: Streaming Startup](#scenario-6-streaming-startup)
   - [Scenario 7: Error Recovery](#scenario-7-error-recovery)
   - [Scenario 8: Disconnection Handling](#scenario-8-disconnection-handling)
3. [TV Brand-Specific Testing](#tv-brand-specific-testing)
4. [Test Checklist](#test-checklist)
5. [Troubleshooting Guide](#troubleshooting-guide)

---

## Test Environment Setup

### Prerequisites

Before testing with real hardware, verify your system meets all requirements:

```bash
# Run doctor checks
swaybeam-doctor

# Or using cargo
cargo run -p swaybeam-doctor
```

### Required Components

| Component | Status Required | Verification Command |
|-----------|-----------------|---------------------|
| Sway/wlroots compositor | OK | `echo $SWAYSOCK` |
| PipeWire daemon | OK | `pgrep pipewire` |
| GStreamer with H.264 | OK | `gst-inspect-1.0 x264` |
| NetworkManager | OK | `nmcli general status` |
| WiFi P2P support | OK | `nmcli device show wlan0` |
| xdg-desktop-portal-wlr | OK | `pgrep xdg-desktop-portal-wlr` |

### Test Hardware Requirements

- **WiFi Adapter**: Must support Wi-Fi Direct/P2P (check with `iw phy phy0 info | grep -i p2p`)
- **Miracast Display**: Any WFD-certified TV, monitor, or projector
- **Network**: Ensure no other WiFi connections interfere (disconnect from regular networks)

### Environment Variables

```bash
# Enable debug logging
export RUST_LOG=debug

# Set interface (adjust based on your system)
export SWAYBEAM_INTERFACE=wlan0

# Increase discovery timeout for slower TVs
export SWAYBEAM_DISCOVERY_TIMEOUT=30
```

---

## Test Scenarios

### Scenario 1: Initial Discovery and Connection

**Objective**: Verify P2P discovery can find Miracast sinks and establish Wi-Fi Direct connection.

#### Prerequisites
- WiFi adapter in P2P mode
- TV with Miracast enabled and visible
- No conflicting WiFi connections

#### Test Procedure

| Step | Action | Expected Result | Timeout |
|------|--------|-----------------|---------|
| 1 | Enable Miracast on TV | TV shows "Waiting for connection" or similar | - |
| 2 | Run `swaybeam-doctor` | All checks pass | 10s |
| 3 | Run discovery: `swaybeam-cli discover` | Lists at least one sink with name and address | 30s |
| 4 | Verify discovered sink info | Sink name matches TV, address format valid | - |
| 5 | Initiate connection: `swaybeam-cli connect <sink>` | Connection request sent, TV prompts for acceptance | 30s |
| 6 | Accept connection on TV | TV shows "Connected" or similar | 10s |
| 7 | Verify P2P group formed | `nmcli device status` shows p2p device | 5s |
| 8 | Check IP assignment | IP address assigned to P2P interface | 5s |

#### Expected Behavior

```
[INFO] Starting P2P discovery on interface wlan0
[INFO] Found P2P device
[INFO] Started P2P discovery...
[INFO] Discovered peer: /org/freedesktop/NetworkManager/Devices/3
[INFO] Found Miracast sink: Samsung TV (AA:BB:CC:DD:EE:FF)
[INFO] Discovery timeout reached
[INFO] Requesting group formation with: Samsung TV (AA:BB:CC:DD:EE:FF)
[INFO] Group formed successfully
[INFO] Connected to sink: Samsung TV
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| No sinks discovered | TV not in Miracast mode | Enable screen mirroring on TV |
| Discovery timeout | WiFi adapter doesn't support P2P | Check `iw phy` for P2P support |
| Connection rejected | TV requires manual acceptance | Accept on TV within timeout |
| Group formation fails | Interference from other networks | Disconnect from WiFi APs |
| No IP assignment | NetworkManager P2P issue | Restart NetworkManager |

#### Debugging Tips

```bash
# Monitor P2P events
nmcli monitor | grep -i p2p

# Check wpa_supplicant logs
journalctl -u wpa_supplicant -f

# Monitor NetworkManager
journalctl -u NetworkManager -f

# Check WiFi capabilities
iw phy phy0 info | grep -A5 "P2P"

# List discovered peers
nmcli device wifi-p2p list
```

---

### Scenario 2: Capability Negotiation

**Objective**: Verify WFD capability exchange between source and sink.

#### Prerequisites
- Active P2P connection established
- RTSP server ready to start

#### Test Procedure

| Step | Action | Expected Result | RTSP Message |
|------|--------|-----------------|--------------|
| 1 | TV initiates RTSP connection | TCP connection on port 7236 | - |
| 2 | TV sends OPTIONS | Server responds with supported methods | `OPTIONS * RTSP/1.0` |
| 3 | Server responds | `200 OK` with Public header | `Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN` |
| 4 | TV sends GET_PARAMETER for wfd_video_formats | Server returns video capabilities | `GET_PARAMETER rtsp://... RTSP/1.0` |
| 5 | Server sends video formats | WFD video format string | `wfd_video_formats: 01 01 00 ...` |
| 6 | TV sends GET_PARAMETER for wfd_audio_codecs | Server returns audio capabilities | `GET_PARAMETER rtsp://... RTSP/1.0` |
| 7 | Server sends audio codecs | AAC/LPCM support indicator | `wfd_audio_codecs: AAC 00000001 00` |
| 8 | TV sends SET_PARAMETER with its capabilities | Server accepts and stores | `SET_PARAMETER rtsp://... RTSP/1.0` |
| 9 | Server responds | `200 OK` confirmation | `RTSP/1.0 200 OK` |

#### Expected RTSP Flow

```text
[TV -> Source] OPTIONS * RTSP/1.0
               CSeq: 1

[Source -> TV] RTSP/1.0 200 OK
               CSeq: 1
               Public: org.wfa.wfd1.0, SET_PARAMETER, GET_PARAMETER

[TV -> Source] GET_PARAMETER rtsp://192.168.10.1 RTSP/1.0
               CSeq: 2

[Source -> TV] RTSP/1.0 200 OK
               CSeq: 2
               Content-Type: text/parameters

               wfd_video_formats: 01 01 00 0000000000000007

[TV -> Source] SET_PARAMETER rtsp://192.168.10.1 RTSP/1.0
               CSeq: 3
               Content-Type: text/parameters

               wfd_video_formats: 01 01 00 000000000000001F
               wfd_audio_codecs: AAC 00000001 00

[Source -> TV] RTSP/1.0 200 OK
               CSeq: 3
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| No RTSP connection | TV didn't initiate | Check TV is in proper mode |
| OPTIONS not received | RTSP server not running | Start RTSP server |
| Invalid video formats | Codec mismatch | Check GStreamer plugins |
| Audio codec rejected | AAC not supported | Try LPCM fallback |

#### Debugging Tips

```bash
# Capture RTSP traffic
tcpdump -i p2p-wlan0-0 -w rtsp.pcap port 7236

# Analyze with Wireshark
wireshark rtsp.pcap

# Monitor RTSP server logs
RUST_LOG=swaybeam_rtsp=debug cargo run -p swaybeam-rtsp

# Test RTSP manually
nc <tv-ip> 7236
OPTIONS * RTSP/1.0
CSeq: 1
```

---

### Scenario 3: Codec Negotiation

**Objective**: Verify proper codec selection based on sink capabilities.

#### Test Variants

| Test | Sink Capability | Expected Codec | Video Format String |
|------|-----------------|----------------|---------------------|
| A | Basic H.264 only | H.264 | `01 01 00 0000000000000007` |
| B | H.264 + H.265 | H.265 (preferred) | `01 01 00 000000000000001F` |
| C | H.265 only | H.265 | `01 01 00 0000000000000010` |
| D | AV1 support | H.264 (fallback) | AV1 not standard yet |

#### Test Procedure

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Receive sink video_formats | Parse codec bitmask |
| 2 | Determine best codec | H.265 if available, else H.264 |
| 3 | Build response format string | Include negotiated codec |
| 4 | Verify GStreamer encoder | Encoder element available |
| 5 | Initialize stream pipeline | Pipeline created successfully |

#### Codec Bitmask Interpretation

```text
Video Format String: "01 01 00 000000000000001F"

Position breakdown:
- 01: Version (WFD 1.0)
- 01: Preferred display mode (native)
- 00: H.264 profile/level indicator
- 000000000000001F: Codec bitmask (hex)

Bitmask interpretation (little-endian):
- Bit 0 (0x0001): H.264 baseline profile
- Bit 1 (0x0002): H.264 main profile
- Bit 2 (0x0004): H.264 high profile
- Bit 4 (0x0010): H.265/HEVC main profile
- Bit 8 (0x0100): AV1 (extension, non-standard)

Example: 0x001F = 00011111
- H.264 baseline (bit 0): YES
- H.264 main (bit 1): YES
- H.264 high (bit 2): YES
- H.265 main (bit 4): YES
=> Negotiate H.265 for best quality
```

#### Test Code

```rust
// Test codec negotiation
use swaybeam_rtsp::WfdCapabilities;

let mut caps = WfdCapabilities::new();

// Test H.264 only
caps.video_formats = Some("01 01 00 0000000000000007".to_string());
assert_eq!(caps.negotiate_video_codec(), NegotiatedCodec::H264);

// Test H.265 available
caps.video_formats = Some("01 01 00 000000000000001F".to_string());
assert_eq!(caps.negotiate_video_codec(), NegotiatedCodec::H265);
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| H.265 negotiation fails | GStreamer x265 missing | Install `gst-plugins-bad` |
| Codec not supported | Missing encoder plugin | Run `gst-inspect-1.0 x265enc` |
| Poor quality with H.264 | Wrong profile selected | Force high profile |

#### Debugging Tips

```bash
# Check available encoders
gst-inspect-1.0 | grep -i enc | grep -E "264|265|av1"

# Test H.264 pipeline
gst-launch-1.0 videotestsrc ! x264enc ! h264parse ! fakesink

# Test H.265 pipeline
gst-launch-1.0 videotestsrc ! x265enc ! h265parse ! fakesink

# Force specific codec in swaybeam
# (modify DaemonConfig or StreamConfig)
```

---

### Scenario 4: Resolution/Framerate Negotiation

**Objective**: Verify proper resolution and framerate selection for different display capabilities.

#### Test Variants

| Test | Resolution | Framerate | Bitrate | Expected Quality |
|------|------------|-----------|---------|------------------|
| A | 1920x1080 | 30fps | 8 Mbps | Standard HD |
| B | 1920x1080 | 60fps | 12 Mbps | HD high-motion |
| C | 3840x2160 | 30fps | 20 Mbps | 4K standard |
| D | 3840x2160 | 60fps | 40 Mbps | 4K high-motion |

#### Test Procedure

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query sink EDID | Get supported resolutions |
| 2 | Match source resolution | Select best available |
| 3 | Configure capture | Set width/height/framerate |
| 4 | Configure encoder | Set bitrate appropriately |
| 5 | Start stream | Pipeline configured correctly |

#### Resolution Configuration

```rust
// Standard HD 1080p
let config_hd = StreamConfig::hd_1080p();
// video_width: 1920, video_height: 1080, framerate: 30

// 4K resolution
let config_4k = StreamConfig::uhd_4k();
// video_width: 3840, video_height: 2160, framerate: 30

// 4K at 60fps (requires H.265)
let config_4k_60 = StreamConfig::uhd_4k_60fps();
// video_codec: H265, bitrate: 40 Mbps
```

#### Bitrate Guidelines

| Resolution | Framerate | Minimum Bitrate | Recommended |
|------------|-----------|-----------------|-------------|
| 720p (1280x720) | 30fps | 2 Mbps | 4 Mbps |
| 1080p (1920x1080) | 30fps | 4 Mbps | 8 Mbps |
| 1080p | 60fps | 6 Mbps | 12 Mbps |
| 4K (3840x2160) | 30fps | 10 Mbps | 20 Mbps |
| 4K | 60fps | 20 Mbps | 40 Mbps |

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| 4K not working | H.265 not negotiated | Ensure H.265 support on TV |
| Low quality | Bitrate too low | Increase bitrate config |
| High latency | Framerate mismatch | Match TV's preferred rate |
| Black screen | Resolution unsupported | Check TV specs |

#### Debugging Tips

```bash
# Test 1080p capture
gst-launch-1.0 videotestsrc ! video/x-raw,width=1920,height=1080,framerate=30/1 ! \
  x264enc bitrate=8000 ! h264parse ! fakesink

# Test 4K capture (requires H.265)
gst-launch-1.0 videotestsrc ! video/x-raw,width=3840,height=2160,framerate=30/1 ! \
  x265enc bitrate=20000 ! h265parse ! fakesink

# Monitor pipeline statistics
gst-launch-1.0 ... ! fakesink enable-last-sample=true
```

---

### Scenario 5: Session Establishment

**Objective**: Verify complete RTSP session establishment including PLAY command.

#### Prerequisites
- Capabilities exchanged
- Codec negotiated
- Parameters agreed upon

#### Test Procedure

| Step | Action | Expected Result | RTSP State |
|------|--------|-----------------|------------|
| 1 | Complete capability exchange | All parameters set | SetParamReceived |
| 2 | TV sends PLAY request | Includes Session header | Play command |
| 3 | Server processes PLAY | Transitions to Play state | SessionState::Play |
| 4 | Server sends RTP-Info | Stream URL and sequence | RTP-Info header |
| 5 | Stream pipeline starts | GStreamer pipeline playing | PipelineState::Playing |
| 6 | First video frames sent | UDP packets to sink IP | RTP streaming |

#### RTSP Session Flow

```text
[TV -> Source] PLAY rtsp://192.168.10.1/stream RTSP/1.0
               CSeq: 5
               Session: sess_12345678

[Source -> TV] RTSP/1.0 200 OK
               CSeq: 5
               Session: sess_12345678
               RTP-Info: url=rtsp://192.168.10.1/movie/, seq=123456

[Streaming begins - RTP packets on port 5004]
```

#### Expected Behavior

```text
[INFO] RTSP negotiation completed
[INFO] Stream pipeline configured
[INFO] Setting output: 192.168.10.2:5004
[INFO] Pipeline started
[INFO] StreamingStarted event sent
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| PLAY rejected | Session not found | Check session ID |
| No RTP-Info | State not ready | Ensure param exchange complete |
| Stream doesn't start | Pipeline error | Check GStreamer logs |
| No video on TV | RTP not received | Check firewall/UDP |

#### Debugging Tips

```bash
# Monitor RTSP session state
RUST_LOG=swaybeam_rtsp=trace cargo run -p swaybeam-daemon

# Check RTP stream
tcpdump -i p2p-wlan0-0 udp port 5004

# Monitor GStreamer pipeline
gst-debug 3 cargo run -p swaybeam-stream

# Test UDP connectivity
nc -u <tv-ip> 5004
```

---

### Scenario 6: Streaming Startup

**Objective**: Verify video streaming begins correctly after PLAY command.

#### Prerequisites
- PLAY command processed
- Stream pipeline initialized
- Capture started

#### Test Procedure

| Step | Action | Expected Result | Verification |
|------|--------|-----------------|--------------|
| 1 | Start screen capture | PipeWire stream active | `pw-cli ls` |
| 2 | Configure GStreamer pipeline | Pipeline in Playing state | Pipeline logs |
| 3 | Set output destination | UDP sink configured | Check udpsink host/port |
| 4 | Push first video buffer | Buffer accepted | AppSrc callback |
| 5 | RTP packets transmitted | UDP traffic on port 5004 | `tcpdump` |
| 6 | TV displays content | Screen mirrored | Visual check |

#### Expected Behavior

```text
[INFO] Capture started successfully
[INFO] PipeWire stream: fd=-1, node_id=1
[INFO] Stream pipeline created
[INFO] Setting output to 192.168.10.2:5004
[INFO] Pipeline state: Playing
[INFO] Pushing video buffers...
```

#### GStreamer Pipeline Elements

```text
appsrc -> videoconvert -> encoder -> parser -> rtp_payloader -> udpsink

For H.264:
appsrc -> videoconvert -> x264enc -> h264parse -> rtph264pay -> udpsink

For H.265:
appsrc -> videoconvert -> x265enc -> h265parse -> rtph265pay -> udpsink
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| Capture fails | Portal not responding | Restart xdg-desktop-portal-wlr |
| Pipeline won't start | Element missing | Install GStreamer plugins |
| No RTP output | UDP blocked | Check firewall rules |
| High latency | Encoder settings | Tune for low latency |
| Poor quality | Bitrate wrong | Adjust encoder bitrate |

#### Debugging Tips

```bash
# Check PipeWire
pw-cli ls
pw-top

# Monitor GStreamer bus
gst-launch-1.0 ... -v

# Check UDP packets
tcpdump -i p2p-wlan0-0 -vv udp port 5004

# Test pipeline manually
gst-launch-1.0 videotestsrc ! x264enc tune=zerolatency ! \
  rtph264pay ! udpsink host=<tv-ip> port=5004

# Check appsrc behavior
# Add debug output in push_video_buffer
```

---

### Scenario 7: Error Recovery

**Objective**: Verify system handles errors gracefully and attempts recovery.

#### Test Variants

| Test | Error Type | Expected Recovery |
|------|------------|-------------------|
| A | Connection drop | Reconnect attempt |
| B | RTSP timeout | Session cleanup, retry |
| C | Pipeline error | Pipeline restart |
| D | Capture failure | Reinitialize capture |
| E | Network loss | Re-establish P2P |

#### Test Procedure: Connection Drop

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Establish streaming session | Streaming active |
| 2 | Simulate connection drop (turn off TV WiFi) | Connection lost event |
| 3 | System detects failure | Error state entered |
| 4 | Cleanup performed | Resources released |
| 5 | Recovery attempt | Reconnection initiated |
| 6 | Connection restored | Streaming resumes |

#### Error Handling Flow

```rust
// Daemon error handling
match self.run_session().await {
    Err(e) => {
        error!("Session failed: {}", e);
        *self.state.write() = DaemonState::Error;
        self.event_tx.send(DaemonEvent::ErrorOccurred(e.to_string()));

        // Cleanup
        let _ = self.stop_stream().await;
        let _ = self.disconnect().await;
    }
}
```

#### Expected Behavior on Error

```text
[ERROR] Session failed: Connection lost
[INFO] Entering Error state
[INFO] Stopping capture
[INFO] Streaming stopped
[INFO] Disconnecting from sink
[INFO] Disconnected from Samsung TV
[INFO] Daemon state: Idle
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| No recovery attempt | Error handler missing | Check daemon error flow |
| Resources not released | Cleanup incomplete | Verify stop_stream/disconnect |
| Reconnection fails | P2P state corrupt | Restart NetworkManager |
| Infinite retry loop | No retry limit | Add max retry count |

#### Debugging Tips

```bash
# Simulate connection drop
nmcli device disconnect p2p-wlan0-0

# Monitor daemon state
watch -n 1 'swaybeam-cli status'

# Check cleanup
ps aux | grep swaybeam
lsof -i :5004

# Force recovery
swaybeam-cli disconnect
swaybeam-cli discover
```

---

### Scenario 8: Disconnection Handling

**Objective**: Verify graceful disconnection from Miracast sink.

#### Test Procedure

| Step | Action | Expected Result | RTSP Message |
|------|--------|-----------------|--------------|
| 1 | Streaming active | Video displayed on TV | - |
| 2 | Initiate disconnect: `swaybeam-cli disconnect` | Disconnect command issued | - |
| 3 | Stop capture | Capture deactivates | - |
| 4 | Stop stream pipeline | Pipeline to Null state | - |
| 5 | Send TEARDOWN to TV | RTSP teardown message | `TEARDOWN rtsp://... RTSP/1.0` |
| 6 | TV responds | `200 OK` confirmation | `RTSP/1.0 200 OK` |
| 7 | Remove RTSP session | Session cleanup | - |
| 8 | Disconnect P2P group | Group removed | - |
| 9 | Return to Idle state | Ready for next connection | - |

#### Expected Behavior

```text
[INFO] Shutting down daemon gracefully...
[INFO] Capture stopped
[INFO] Streaming stopped
[INFO] Disconnected from Samsung TV
[INFO] P2P connection terminated
[INFO] Daemon state: Idle
```

#### TEARDOWN Flow

```text
[Source -> TV] TEARDOWN rtsp://192.168.10.1/stream RTSP/1.0
               CSeq: 6
               Session: sess_12345678

[TV -> Source] RTSP/1.0 200 OK
               CSeq: 6

[Session removed, connection terminated]
```

#### Common Failure Points

| Failure | Likely Cause | Resolution |
|---------|--------------|------------|
| TEARDOWN not sent | Session missing | Verify session exists |
| TV doesn't respond | Already disconnected | Handle gracefully |
| P2P remains | Group not stopped | Call `p2p.stop()` |
| Resources leaked | Cleanup incomplete | Check Drop implementations |

#### Debugging Tips

```bash
# Monitor disconnect flow
RUST_LOG=swaybeam_daemon=debug swaybeam-cli disconnect

# Verify P2P cleanup
nmcli device wifi-p2p list
nmcli device status

# Check no lingering processes
lsof -i :7236
lsof -i :5004

# Verify session removed
swaybeam-cli status
```

---

## TV Brand-Specific Testing

### Samsung Smart TVs

#### Models Tested
- Samsung QLED series (2019+)
- Samsung Crystal series
- Samsung The Frame

#### Samsung-Specific Behavior

| Parameter | Samsung Value | Notes |
|-----------|---------------|-------|
| wfd_video_formats | `01 01 02 000000000000001F` | Supports H.264 and H.265 |
| wfd_audio_codecs | `AAC 00000002 00` | AAC preferred |
| Discovery response | Quick (~5s) | Fast discovery |
| Connection prompt | Popup on screen | Requires acceptance |
| HDCP support | Yes | Content protection required |

#### Samsung Test Checklist

```
[ ] TV in Screen Mirroring mode (Settings > Connection > Screen Mirroring)
[ ] Discovery finds "Samsung TV" or similar name
[ ] Connection prompts for PIN (if configured)
[ ] H.265 negotiated for 4K models
[ ] Streaming works at 1080p and 4K
[ ] Disconnection handled gracefully
```

#### Samsung Known Issues

- HDCP may reject protected content
- Some models require PIN entry
- Older models may not support 4K@60fps

---

### LG Smart TVs

#### Models Tested
- LG OLED series (2019+)
- LG NanoCell series
- LG UHD series

#### LG-Specific Behavior

| Parameter | LG Value | Notes |
|-----------|----------|-------|
| wfd_video_formats | `01 01 01 0000000000000007` | H.264 primary |
| wfd_audio_codecs | `AAC 00000001 00 LPCM 00000001 00` | AAC or LPCM |
| Discovery response | Medium (~10s) | Slower discovery |
| Connection prompt | Bottom banner | Auto-accept option available |
| 4K support | Limited | Check model specs |

#### LG Test Checklist

```
[ ] TV in Screen Share mode (Home > Screen Share)
[ ] Discovery finds "LG TV" or webOS name
[ ] Connection may auto-accept or prompt
[ ] H.264 negotiated
[ ] AAC audio confirmed
[ ] Works at 1080p (4K may need H.265 check)
[ ] Disconnection returns TV to normal mode
```

#### LG Known Issues

- WebOS 3.0+ has better Miracast support
- Some models prefer HDCP 2.2
- Audio sync issues on some models

---

### Sony Bravia TVs

#### Models Tested
- Sony Bravia XR series
- Sony Bravia X90 series
- Sony Bravia A80/A90 OLED

#### Sony-Specific Behavior

| Parameter | Sony Value | Notes |
|-----------|------------|-------|
| wfd_video_formats | `01 01 02 000000000000001F` | Full codec support |
| wfd_audio_codecs | `AAC 00000003 00` | AAC with extended modes |
| Discovery response | Variable (5-15s) | Depends on model |
| Connection prompt | Side notification | Quick accept |
| EDID detailed | Yes | Full resolution info |

#### Sony Test Checklist

```
[ ] TV in Screen Mirroring (Input > Screen Mirroring)
[ ] Discovery finds "BRAVIA" or Sony model name
[ ] Connection prompts briefly
[ ] H.265 available on 4K models
[ ] Full 4K@60fps on supported models
[ ] EDID provides resolution options
[ ] UIBC may be supported (remote control back-channel)
```

#### Sony Known Issues

- Android TV version affects behavior
- Some models have UIBC support
- Requires "Network and Internet" setup first

---

### Generic/Other Brands

#### Common Test Parameters

| Brand | Typical Behavior | Notes |
|-------|------------------|-------|
| Vizio | H.264 only | Basic Miracast |
| TCL | Variable | Check Android TV version |
| Hisense | H.264/H.265 | Newer models support 4K |
| Philips | H.264 | Android TV based |
| Projectors | Variable | Often basic support |

#### Generic Test Checklist

```
[ ] Enable "Wireless Display" or "Miracast" in settings
[ ] Discovery timeout increased (30s+)
[ ] Basic H.264 tested first
[ ] Manual resolution adjustment if needed
[ ] Lower bitrate for less capable devices
[ ] Test audio separately
```

---

## Test Checklist

### Pre-Test Checklist

```
System Environment:
[ ] Run swaybeam-doctor - all checks pass
[ ] WiFi adapter supports P2P (iw phy phy0 info | grep P2P)
[ ] NetworkManager running (systemctl status NetworkManager)
[ ] PipeWire running (systemctl --user status pipewire)
[ ] GStreamer plugins installed (gst-inspect-1.0 x264, x265)
[ ] xdg-desktop-portal-wlr running (pgrep xdg-desktop-portal-wlr)
[ ] Sway session active (echo $SWAYSOCK)

TV/Display Setup:
[ ] TV Miracast mode enabled
[ ] TV on same network segment (no router interference)
[ ] TV firmware updated
[ ] HDCP content test content prepared
[ ] Test content (video, images) prepared

Network Setup:
[ ] Disconnect from WiFi APs (avoid interference)
[ ] No Bluetooth devices interfering
[ ] Firewall allows UDP 5004 and TCP 7236
[ ] WiFi channel clear (minimal interference)
```

### Connection Test Checklist

```
Discovery Phase:
[ ] swaybeam-cli discover lists at least one sink
[ ] Sink name matches TV brand/model
[ ] Sink address is valid MAC format
[ ] Discovery completes within 30 seconds

Connection Phase:
[ ] swaybeam-cli connect initiates connection
[ ] TV prompts for acceptance (if required)
[ ] Connection accepted within 15 seconds
[ ] P2P group formed (nmcli device status)
[ ] IP address assigned to P2P interface
[ ] RTSP connection established on port 7236
```

### Negotiation Test Checklist

```
RTSP Negotiation:
[ ] OPTIONS command received and responded
[ ] GET_PARAMETER for video formats exchanged
[ ] GET_PARAMETER for audio codecs exchanged
[ ] SET_PARAMETER with sink capabilities received
[ ] Session ID assigned
[ ] Codec properly negotiated (check logs)

Capability Verification:
[ ] Video codec matches TV capabilities
[ ] Audio codec matches TV capabilities
[ ] Resolution appropriate for TV
[ ] Framerate appropriate for content
```

### Streaming Test Checklist

```
Stream Startup:
[ ] PLAY command received and responded
[ ] Stream pipeline started (GStreamer Playing state)
[ ] Capture started (PipeWire stream active)
[ ] UDP packets transmitted (tcpdump shows traffic)
[ ] TV displays mirrored content

Quality Verification:
[ ] Video quality acceptable (no artifacts)
[ ] Audio synchronized (no delay/lag)
[ ] Framerate stable (no drops)
[ ] Latency acceptable (<500ms for most use)
[ ] No buffer underruns
```

### Stability Test Checklist

```
Duration Test:
[ ] Stream runs for 5 minutes without issues
[ ] Stream runs for 30 minutes without issues
[ ] Stream runs for 1 hour without issues
[ ] No memory leaks (monitor system memory)
[ ] No CPU overload (monitor CPU usage)

Error Recovery Test:
[ ] Connection drop handled gracefully
[ ] Manual disconnect handled correctly
[ ] TV disconnect handled correctly
[ ] Recovery attempt succeeds (if applicable)
```

### Cleanup Test Checklist

```
Disconnection:
[ ] swaybeam-cli disconnect works
[ ] TEARDOWN sent and acknowledged
[ ] Pipeline stops cleanly
[ ] Capture stops cleanly
[ ] P2P group removed
[ ] All resources released
[ ] System returns to idle state

Post-Test:
[ ] No lingering processes
[ ] No open ports (check netstat)
[ ] System stable for next test
```

---

## Troubleshooting Guide

### Discovery Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| No sinks found | Discovery returns empty list | Enable Miracast on TV, check P2P hardware |
| Discovery slow | Timeout exceeded | Increase timeout, check WiFi interference |
| Wrong sink type | Non-Miracast device listed | Filter by WFD IEs in discovery |

**Commands**:
```bash
iw phy phy0 info | grep -i p2p
nmcli device wifi-p2p list
journalctl -u NetworkManager -f
```

### Connection Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| Connection rejected | TV says "Connection failed" | Accept on TV, check credentials |
| Timeout during connect | No group formed | Restart NetworkManager, check wpa_supplicant |
| No IP address | 0.0.0.0 or None | NetworkManager P2P config issue |

**Commands**:
```bash
nmcli general status
nmcli connection show
wpa_cli p2p_peer
```

### RTSP Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| No OPTIONS | RTSP never starts | TV didn't initiate, check connection |
| Invalid response | 400/500 errors | Check message format in logs |
| Session lost | 454 Session Not Found | Session cleanup bug, restart |

**Commands**:
```bash
tcpdump -i p2p-wlan0-0 port 7236 -w rtsp.pcap
wireshark rtsp.pcap
RUST_LOG=swaybeam_rtsp=trace cargo run
```

### Streaming Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| No video on TV | Black screen | Check UDP traffic, verify RTP |
| Poor quality | Artifacts, blocky | Increase bitrate, check encoder |
| High latency | Delay >1s | Tune encoder for low latency |
| Audio issues | No audio or sync issues | Check audio codec, bitrate |

**Commands**:
```bash
tcpdump -i p2p-wlan0-0 udp port 5004
gst-launch-1.0 ... -v
pw-top
```

### GStreamer Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| Pipeline won't start | State transition failed | Missing plugin or wrong config |
| Encoder error | Element won't link | Check encoder available, install plugins |
| No RTP output | udpsink silent | Check host/port, network reachability |

**Commands**:
```bash
gst-inspect-1.0 x264enc
gst-inspect-1.0 x265enc
gst-launch-1.0 videotestsrc ! x264enc ! fakesink -v
```

### Capture Issues

| Problem | Symptoms | Solution |
|---------|----------|----------|
| Portal timeout | Capture start hangs | Restart xdg-desktop-portal-wlr |
| Permission denied | Portal rejected | Grant screen sharing permission |
| PipeWire error | No stream | Check PipeWire, restart if needed |

**Commands**:
```bash
systemctl --user restart xdg-desktop-portal-wlr
pw-cli ls
busctl --user list | grep portal
```

---

## Appendix A: RTSP Message Reference

### OPTIONS Request/Response

```text
Request:
OPTIONS * RTSP/1.0
CSeq: 1

Response:
RTSP/1.0 200 OK
CSeq: 1
Public: org.wfa.wfd1.0, SET_PARAMETER, GET_PARAMETER, PLAY, TEARDOWN
```

### GET_PARAMETER Request/Response

```text
Request:
GET_PARAMETER rtsp://192.168.10.1 RTSP/1.0
CSeq: 2

Response:
RTSP/1.0 200 OK
CSeq: 2
Content-Type: text/parameters

wfd_video_formats: 01 01 00 0000000000000007
wfd_audio_codecs: AAC 00000001 00
wfd_client_rtp_ports: RTP/AVP/UDP;unicast 19000 0 mode=play
```

### SET_PARAMETER Request/Response

```text
Request:
SET_PARAMETER rtsp://192.168.10.1 RTSP/1.0
CSeq: 3
Content-Type: text/parameters

wfd_video_formats: 01 01 00 000000000000001F
wfd_audio_codecs: AAC 00000001 00

Response:
RTSP/1.0 200 OK
CSeq: 3
```

### PLAY Request/Response

```text
Request:
PLAY rtsp://192.168.10.1/stream RTSP/1.0
CSeq: 4
Session: sess_12345678

Response:
RTSP/1.0 200 OK
CSeq: 4
Session: sess_12345678
RTP-Info: url=rtsp://192.168.10.1/movie/, seq=123456
```

### TEARDOWN Request/Response

```text
Request:
TEARDOWN rtsp://192.168.10.1/stream RTSP/1.0
CSeq: 5
Session: sess_12345678

Response:
RTSP/1.0 200 OK
CSeq: 5
```

---

## Appendix B: WFD Parameter Reference

### wfd_video_formats Format

```text
Format: <version> <display_mode> <h264_codec> <codec_bitmask>

version: 01 (WFD 1.0)
display_mode: 01 (native), 02 (portrait), etc.
h264_codec: 00 (CEA profile), 01 (VESA profile), etc.
codec_bitmask: 16-digit hex bitmask of supported codecs
```

### wfd_audio_codecs Format

```text
Format: <codec_name> <modes> <latency>

codec_name: AAC, LPCM
modes: hex bitmask of supported modes
latency: latency class (00, 01, 02)
```

### wfd_client_rtp_ports Format

```text
Format: RTP/AVP/UDP;unicast <port> <port2> mode=<mode>

Example: RTP/AVP/UDP;unicast 19000 0 mode=play
```

---

## Appendix C: GStreamer Pipeline Examples

### H.264 1080p Pipeline

```bash
gst-launch-1.0 \
  videotestsrc ! \
  video/x-raw,width=1920,height=1080,framerate=30/1 ! \
  videoconvert ! \
  x264enc bitrate=8000 tune=zerolatency speed-preset=veryfast ! \
  h264parse ! \
  rtph264pay ! \
  udpsink host=192.168.10.2 port=5004 sync=false async=false
```

### H.265 4K Pipeline

```bash
gst-launch-1.0 \
  videotestsrc ! \
  video/x-raw,width=3840,height=2160,framerate=30/1 ! \
  videoconvert ! \
  x265enc bitrate=20000 tune=zerolatency speed-preset=fast ! \
  h265parse ! \
  rtph265pay ! \
  udpsink host=192.168.10.2 port=5004 sync=false async=false
```

### AV1 Pipeline (if supported)

```bash
gst-launch-1.0 \
  videotestsrc ! \
  video/x-raw,width=1920,height=1080,framerate=30/1 ! \
  videoconvert ! \
  svtav1enc preset=8 target-bitrate=8000 ! \
  av1parse ! \
  rtpav1pay ! \
  udpsink host=192.168.10.2 port=5004 sync=false async=false
```

---

## Appendix D: NetworkManager P2P Commands

### Discovery Commands

```bash
# Check P2P capability
nmcli general status | grep WIFI-P2P

# List P2P devices
nmcli device wifi-p2p list

# Start P2P find
nmcli device wifi-p2p start-find wlan0

# Stop P2P find
nmcli device wifi-p2p stop-find wlan0
```

### Connection Commands

```bash
# Connect to peer
nmcli device wifi-p2p connect wlan0 <peer-mac>

# Check connection status
nmcli device status

# Disconnect
nmcli device disconnect p2p-wlan0-0
```

---

## Document Version

- **Version**: 1.0.0
- **Last Updated**: 2026-04-02
- **Compatible with**: swaybeam v0.1.0+

---

## Contributing

To contribute additional test scenarios or TV brand specifics:

1. Test with real hardware
2. Document the exact RTSP message flow
3. Note any brand-specific quirks
4. Submit as PR to docs/MIRACAST_TEST_SCENARIOS.md
