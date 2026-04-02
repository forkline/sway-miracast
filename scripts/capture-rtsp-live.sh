#!/bin/bash
# Real-time RTSP message monitoring (ASCII)
# Usage: ./scripts/capture-rtsp-live.sh

echo "Monitoring RTSP traffic in real-time..."
echo "Showing message contents on port 7236"
echo "Press Ctrl+C to stop"

sudo tcpdump -i any -nn -A 'host 192.168.49.1 and port 7236'
