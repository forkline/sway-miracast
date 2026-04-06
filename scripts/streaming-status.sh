#!/bin/bash
# Quick check if streaming is working

TV_IP="192.168.49.1"

# Check if we can ping TV
if ! ping -c 1 -W 1 $TV_IP > /dev/null 2>&1; then
    echo "❌ Not connected to TV"
    exit 1
fi

# Check RTP packets on P2P interface
INTERFACE=$(ip route get $TV_IP 2>/dev/null | grep -oP 'dev \K\S+')
if [ -z "$INTERFACE" ]; then
    echo "❌ No P2P interface found"
    exit 1
fi

# Count UDP packets in 2 seconds
PACKETS=$(sudo timeout 2 tcpdump -i $INTERFACE -c 50 'udp' 2>/dev/null | grep -c "UDP" || echo "0")

if [ "$PACKETS" -gt 10 ]; then
    echo "✅ Streaming active: $PACKETS UDP packets/sec"
    # Check TV process
    TV_PROC=$(ssh root@hall-tv "ps aux | grep -v grep | grep -c miracast" 2>/dev/null || echo "0")
    if [ "$TV_PROC" -gt 0 ]; then
        echo "✅ TV miracast process running"
    fi
    exit 0
else
    echo "❌ No streaming: $PACKETS UDP packets"
    exit 1
fi
