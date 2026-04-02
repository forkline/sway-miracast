#!/bin/bash
# Wait for P2P interface to appear, then capture session
# Usage: ./scripts/capture-p2p.sh [output-file]

set -e

OUTPUT="${1:-swaybeam-session.pcap}"
echo "Monitoring for P2P interface creation..."
echo "Will capture to: $OUTPUT"

# Watch for p2p interface
while true; do
  P2P_IFACE=$(ip -4 -o addr show | grep -E "^[0-9]+: (p2p-|p2p0)" | awk '{print $2}' | head -1)
  if [ -n "$P2P_IFACE" ]; then
    echo "Found P2P interface: $P2P_IFACE"
    IP=$(ip -4 -o addr show "$P2P_IFACE" | awk '{print $4}' | cut -d/ -f1)
    echo "Local IP: $IP"
    break
  fi
  sleep 0.5
done

# Start capture
echo "Starting capture on $P2P_IFACE..."
echo "Capturing all traffic to/from 192.168.49.1 (LG TV)"
echo "Press Ctrl+C to stop capture"
sudo tcpdump -i "$P2P_IFACE" -w "$OUTPUT" host 192.168.49.1

echo "Capture saved to $OUTPUT"
