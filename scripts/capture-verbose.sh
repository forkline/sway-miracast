#!/bin/bash
# Verbose capture with hex dump for deep protocol debugging
# Usage: ./scripts/capture-verbose.sh [interface] [output-file]

set -e

IFACE="${1:-p2p-wlp2s0-7}"
OUTPUT="${2:-swaybeam-verbose.pcap}"

echo "Verbose capture on $IFACE..."
echo "Including full hex dump for TCP traffic"
echo "Output: $OUTPUT"
echo "Press Ctrl+C to stop"

sudo tcpdump -i "$IFACE" \
  -w "$OUTPUT" \
  -vv -XX \
  'host 192.168.49.1 and tcp'

echo "Capture saved to $OUTPUT"
