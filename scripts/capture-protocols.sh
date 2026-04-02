#!/bin/bash
# Capture RTSP, HDCP, and RTP traffic on P2P interface
# Usage: ./scripts/capture-protocols.sh [interface] [output-file]

set -e

IFACE="${1:-p2p-wlp2s0-7}"
OUTPUT="${2:-swaybeam-protocols.pcap}"

echo "Capturing protocol traffic on $IFACE..."
echo "Ports: RTSP(7236), HDCP(53002), RTP(53000-53010)"
echo "Output: $OUTPUT"
echo "Press Ctrl+C to stop"

sudo tcpdump -i "$IFACE" \
  -w "$OUTPUT" \
  'host 192.168.49.1 and (port 7236 or port 53002 or portrange 53000-53010)'

echo "Capture saved to $OUTPUT"
echo "Analyze with: wireshark $OUTPUT"
