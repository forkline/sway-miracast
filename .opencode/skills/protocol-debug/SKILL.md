---
name: protocol-debug
description: Use Wireshark/tcpdump capture scripts to debug Miracast/WFD/RTSP/HDCP protocol issues with LG TVs and other sinks.
---

## Purpose

Provide fast, evidence-based protocol debugging using packet capture to identify exact message sequences, timing issues, and protocol mismatches when connecting swaybeam to Miracast sinks (especially LG webOS TVs).

## When to use

Use this skill when:
- RTSP negotiation fails (PLAY rejected, connection reset)
- HDCP handshake stalls or fails
- P2P/Wi-Fi Direct connection succeeds but streaming fails
- Need to compare actual traffic vs expected protocol behavior
- User asks to "debug faster" or "see what's actually happening"
- Protocol-level debugging is needed (not just log analysis)
- Need evidence of what bytes/messages are exchanged
- Investigating LG-specific reverse RTSP or HDCP behavior

## Why packet capture

Logs show what we *think* happened. Packet capture shows what *actually* happened:
- Exact byte sequences
- Message timing and ordering
- Missing or unexpected messages
- Connection state transitions
- Protocol violations by either side

This is especially critical for LG webOS TVs which:
- Use reverse RTSP (TV connects to us, not vice versa)
- Require full HDCP 2.1 handshake
- May send unexpected message sequences

## Available Scripts

All scripts are in `scripts/` directory and are executable.

### 1. capture-p2p.sh (Auto-detect and capture)

**Purpose:** Monitor for P2P interface creation, automatically start capture.

**Usage:**
```bash
# Terminal 1: Start capture (will wait for P2P interface)
./scripts/capture-p2p.sh [output-file]

# Terminal 2: Run swaybeam daemon
cargo run -p swaybeam-cli --bin swaybeam -- daemon --sink "22:28:BC:A8:6C:FE" --client
```

**What it does:**
- Monitors for P2P interface creation (e.g., `p2p-wlp2s0-7`)
- Auto-detects interface and local IP
- Starts tcpdump capture to specified file
- Captures all traffic to/from LG TV (192.168.49.1)

**Output:** pcap file for Wireshark analysis

**Best for:** Full session capture when you don't know interface name

---

### 2. capture-protocols.sh (Protocol-specific capture)

**Purpose:** Capture specific protocol ports (RTSP, HDCP, RTP).

**Usage:**
```bash
./scripts/capture-protocols.sh [interface] [output-file]

# Example:
./scripts/capture-protocols.sh p2p-wlp2s0-7 swaybeam-protocols.pcap
```

**What it captures:**
- RTSP: port 7236
- HDCP: port 53002
- RTP: ports 53000-53010

**Best for:** Focused capture on known interface, excludes unrelated traffic

---

### 3. capture-verbose.sh (Detailed hex dump)

**Purpose:** Capture with full hex output for byte-level analysis.

**Usage:**
```bash
./scripts/capture-verbose.sh [interface] [output-file]
```

**What it does:**
- Captures all TCP traffic with `-vv -XX` flags
- Shows full hex dump of every packet
- Most detailed capture for deep debugging

**Best for:** Analyzing exact message structure, debugging malformed packets

---

### 4. analyze-pcap.sh (Quick analysis helper)

**Purpose:** Extract key protocol info from captured pcap without opening Wireshark.

**Usage:**
```bash
./scripts/analyze-pcap.sh [pcap-file]

# Example:
./scripts/analyze-pcap.sh swaybeam-session.pcap
```

**What it shows:**
- RTSP message sequence (OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY)
- HDCP message sequence (AKE_Init, AKE_Send_Cert, etc.)
- RTP packet count
- TCP connection analysis (resets, retransmits)
- Message counts per protocol
- Wireshark filter suggestions

**Best for:** Quick diagnosis without full Wireshark analysis

---

### 5. capture-rtsp-live.sh (Real-time RTSP monitor)

**Purpose:** Watch RTSP messages in real-time as ASCII text.

**Usage:**
```bash
./scripts/capture-rtsp-live.sh
```

**What it shows:**
- Live RTSP message contents on port 7236
- ASCII dump shows request/response bodies
- Immediate visibility of negotiation steps

**Best for:** Live debugging, seeing RTSP messages as they happen

---

### 6. capture-hdcp-live.sh (Real-time HDCP hex monitor)

**Purpose:** Watch HDCP messages as hex dump in real-time.

**Usage:**
```bash
./scripts/capture-hdcp-live.sh
```

**What it shows:**
- Live HDCP traffic on port 53002
- Hex dump shows message IDs and payloads
- Can identify message types by first byte (msg_id)

**Best for:** Live HDCP debugging, seeing handshake progression

---

## Protocol Reference

### RTSP Sequence (Normal WFD)

Expected sequence:
1. OPTIONS (C->S or S->C in reverse mode)
2. GET_PARAMETER (get sink capabilities)
3. SET_PARAMETER (set source capabilities)
4. SETUP (establish RTP stream) - **skipped in LG reverse mode**
5. PLAY (start streaming)

**LG Reverse Mode:** TV connects to us on port 7236, we act as server. SETUP is skipped, PLAY sent directly to `/stream`.

### HDCP 2.1 Sequence

Expected sequence:
1. AKE_Init (msg_id=2, source sends 8-byte r_tx)
2. AKE_Send_Cert (msg_id=3, receiver sends 522-byte cert + repeater flag)
3. AKE_Transmitter_Info (msg_id=20, optional)
4. AKE_No_Stored_km (msg_id=4, source sends encrypted Km)
5. AKE_Receiver_Info (msg_id=14, receiver sends version/capabilities)
6. **MISSING?** AKE_Send_rrx (msg_id=6, 8-byte r_rx)
7. **MISSING?** AKE_Send_H_prime (msg_id=7, 32-byte H')
8. **MISSING?** AKE_Send_Pairing_Info (msg_id=8, optional pairing info)
9. LC_Init (msg_id=9, locality check)
10. LC_Send_L_prime (msg_id=10, verify locality)
11. SKE_Send_Eks (msg_id=11, session key exchange)
12. RTSP PLAY allowed after HDCP complete

**Key insight:** We currently reach step 5 (AKE_Receiver_Info) but may be missing steps 6-8 before PLAY.

### Message ID Reference

HDCP message IDs (first byte of each message):
- 2: AKE_Init
- 3: AKE_Send_Cert
- 4: AKE_No_Stored_km
- 6: AKE_Send_rrx
- 7: AKE_Send_H_prime
- 8: AKE_Send_Pairing_Info
- 9: LC_Init
- 10: LC_Send_L_prime
- 11: SKE_Send_Eks
- 14: AKE_Receiver_Info
- 20: AKE_Transmitter_Info

## Workflow

### Basic Capture Workflow

```bash
# Step 1: Start capture
./scripts/capture-p2p.sh

# Step 2: Run swaybeam (in another terminal)
cargo run -p swaybeam-cli --bin swaybeam -- daemon --sink "22:28:BC:A8:6C:FE" --client

# Step 3: Let it fail, stop capture (Ctrl+C)

# Step 4: Quick analysis
./scripts/analyze-pcap.sh swaybeam-session.pcap

# Step 5: Deep analysis
wireshark swaybeam-session.pcap
```

### Live Monitoring Workflow

```bash
# Terminal 1: Watch RTSP
./scripts/capture-rtsp-live.sh

# Terminal 2: Watch HDCP
./scripts/capture-hdcp-live.sh

# Terminal 3: Run swaybeam
cargo run -p swaybeam-cli --bin swaybeam -- daemon --sink "22:28:BC:A8:6C:FE" --client

# Watch messages appear in real-time
```

### Focused Protocol Capture

```bash
# Step 1: Find interface name (run swaybeam first, note interface from logs)
# Interface will be something like: p2p-wlp2s0-7, p2p-wlan0-5, or p2p0

# Step 2: Capture specific ports
./scripts/capture-protocols.sh p2p-wlp2s0-7 session.pcap

# Step 3: Run swaybeam in another terminal

# Step 4: Analyze
./scripts/analyze-pcap.sh session.pcap
wireshark session.pcap
```

## Wireshark Analysis Tips

### Useful Filters

```
tcp.port == 7236              # RTSP traffic
tcp.port == 53002             # HDCP traffic
udp.port >= 53000             # RTP traffic
tcp.analysis.flags            # Connection issues (reset, retransmit)
tcp.analysis.reset            # Just connection resets
tcp.stream == 0               # First TCP stream (usually RTSP)
tcp.stream == 1               # Second stream (usually HDCP)
```

### What to Look For

**RTSP Analysis:**
1. Message sequence (follow TCP stream)
2. Timing between messages
3. PLAY response (or reset)
4. Any unexpected messages from TV
5. Request/response formatting

**HDCP Analysis:**
1. Message IDs (first byte of each message)
2. Message lengths (compare against spec)
3. Missing messages (especially after AKE_Receiver_Info)
4. Timing: do we send PLAY before HDCP completes?
5. Connection state when PLAY is sent

**RTP Analysis:**
1. Any RTP packets sent before PLAY succeeds?
2. Port numbers used
3. Packet structure

**Connection Issues:**
1. TCP resets (which side initiates?)
2. Timing of reset relative to protocol state
3. Retransmissions (indicates connection problems)

## Common Findings

### Missing HDCP Messages

Symptom: PLAY rejected after AKE_Receiver_Info

Analysis:
- Check if LG sends AKE_Send_rrx (msg_id=6) after our AKE_No_Stored_km
- Check if LG sends AKE_Send_H_prime (msg_id=7)
- Check if we process them correctly
- Check timing: do we race into PLAY without waiting?

### RTSP Timing Race

Symptom: PLAY sent before HDCP handshake completes

Analysis:
- Look at HDCP socket state when PLAY is sent
- Check if HDCP socket is still receiving messages
- Compare message timestamps vs PLAY timestamp
- May need to wait for specific HDCP completion signal

### Message Format Issues

Symptom: LG rejects specific HDCP message

Analysis:
- Compare our message structure vs spec
- Check byte ordering
- Verify lengths (especially AKE_No_Stored_km should be 128 bytes)
- Check RSA encryption padding (should be OAEP)

### Reverse RTSP Mode Issues

Symptom: RTSP negotiation behaves unexpectedly

Analysis:
- Check which side initiates connection (should be TV to us)
- Check interleaved messages (TV may send OPTIONS on our socket)
- Verify we handle both directions correctly
- Check SETUP is skipped (LG reverse mode)

## Expected Output

After running capture workflow, you should have:
1. pcap file with full session traffic
2. Quick analysis showing message counts and sequences
3. Wireshark view showing detailed byte-level traffic
4. Clear evidence of what's missing or wrong

## Comparison with Reference Implementation

If possible, capture traffic from a working Miracast source (GNOME, Windows) connecting to the same TV:

```bash
# Capture GNOME session
./scripts/capture-p2p.sh gnome-reference.pcap
# Run gnome-network-displays or similar

# Compare
wireshark gnome-reference.pcap
wireshark swaybeam-session.pcap

# Look for differences in:
# - Message sequence
# - Message timing
# - Message structure
# - Number of messages
```

This shows exactly what a working source does differently.

## Quick Reference

| Task | Command |
|------|---------|
| Auto-capture session | `./scripts/capture-p2p.sh` |
| Capture specific ports | `./scripts/capture-protocols.sh <iface>` |
| Verbose hex capture | `./scripts/capture-verbose.sh <iface>` |
| Quick analysis | `./scripts/analyze-pcap.sh <file>` |
| Live RTSP monitor | `./scripts/capture-rtsp-live.sh` |
| Live HDCP monitor | `./scripts/capture-hdcp-live.sh` |
| Open in Wireshark | `wireshark <file>` |

## Checklist

- [ ] Identified protocol debugging need (RTSP/HDCP failure)
- [ ] Chose appropriate capture script
- [ ] Started capture before running swaybeam
- [ ] Captured full failed session
- [ ] Ran analyze-pcap.sh for quick diagnosis
- [ ] Opened in Wireshark for deep analysis
- [ ] Identified missing/malformed messages
- [ ] Identified timing issues
- [ ] Compared against expected protocol sequence
- [ ] Documented findings
- [ ] Proposed fix based on evidence

## Integration with swaybeam Development

After capture analysis:
1. Document findings in issue/PR description
2. Link to pcap file (upload to GitHub issue if needed)
3. Implement fix based on evidence
4. Re-capture to verify fix works
5. Commit both fix and any new debug improvements

## Example Session

```bash
# User: "PLAY is being rejected, debug faster"

# AI: "Let me start a packet capture to see exactly what's happening"

# Terminal 1
$ ./scripts/capture-p2p.sh
Monitoring for P2P interface creation...
Found P2P interface: p2p-wlp2s0-7
Local IP: 192.168.49.10
Starting capture...
[Waiting for swaybeam]

# Terminal 2
$ cargo run -p swaybeam-cli --bin swaybeam -- daemon --sink "22:28:BC:A8:6C:FE" --client
[swaybeam runs and fails]

# Terminal 1
[Ctrl+C to stop capture]
Capture saved to swaybeam-session.pcap

# Terminal 1
$ ./scripts/analyze-pcap.sh swaybeam-session.pcap
=== RTSP Traffic ===
OPTIONS -> GET_PARAMETER -> SET_PARAMETER -> PLAY -> [RESET]

=== HDCP Traffic ===
AKE_Init (2) -> AKE_Send_Cert (3) -> AKE_Transmitter_Info (20) ->
AKE_No_Stored_km (4) -> AKE_Receiver_Info (14) -> [no further messages]

=== Connection Analysis ===
TCP reset from 192.168.49.1 after PLAY

# AI: "Analysis shows HDCP stops at AKE_Receiver_Info. LG likely expects
      AKE_Send_H_prime (msg_id=7) before accepting PLAY. Need to wait for
      additional HDCP messages."

# Terminal 1
$ wireshark swaybeam-session.pcap
[Deep analysis confirms timing race]
```

## Key Takeaways

1. **Capture first, hypothesize second** - Evidence-based debugging
2. **Compare byte sequences** - Exact message structure matters
3. **Check timing** - Race conditions are common
4. **Look for missing messages** - Protocol may expect more than we implement
5. **Use live monitoring** - For real-time feedback
6. **Document findings** - Link pcap evidence to fixes
