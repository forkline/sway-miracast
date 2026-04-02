#!/bin/bash
# Quick analysis of captured Miracast session
# Usage: ./scripts/analyze-pcap.sh [pcap-file]

set -e

PCAP="${1:-swaybeam-session.pcap}"

if [ ! -f "$PCAP" ]; then
  echo "Error: File not found: $PCAP"
  exit 1
fi

echo "=== RTSP Traffic (port 7236) ==="
sudo tcpdump -r "$PCAP" -nn -A 'port 7236' 2>/dev/null | head -150

echo ""
echo "=== HDCP Traffic (port 53002) ==="
sudo tcpdump -r "$PCAP" -nn -XX 'port 53002' 2>/dev/null | head -100

echo ""
echo "=== RTP Traffic (ports 53000-53010) ==="
sudo tcpdump -r "$PCAP" -nn 'portrange 53000-53010' 2>/dev/null | head -50

echo ""
echo "=== Connection Analysis (TCP issues) ==="
sudo tcpdump -r "$PCAP" -nn 'tcp.analysis' 2>/dev/null | head -50

echo ""
echo "=== Message Counts ==="
echo "RTSP packets: $(sudo tcpdump -r "$PCAP" -nn 'port 7236' 2>/dev/null | wc -l)"
echo "HDCP packets: $(sudo tcpdump -r "$PCAP" -nn 'port 53002' 2>/dev/null | wc -l)"
echo "RTP packets:  $(sudo tcpdump -r "$PCAP" -nn 'portrange 53000-53010' 2>/dev/null | wc -l)"

echo ""
echo "=== Next Steps ==="
echo "Open in Wireshark for detailed analysis:"
echo "  wireshark $PCAP"
echo ""
echo "Useful Wireshark filters:"
echo "  tcp.port == 7236        # RTSP"
echo "  tcp.port == 53002       # HDCP"
echo "  udp.port >= 53000       # RTP"
echo "  tcp.analysis.flags      # Connection issues"
