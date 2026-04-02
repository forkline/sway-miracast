#!/bin/bash
# Real-time HDCP message monitoring (hex dump)
# Usage: ./scripts/capture-hdcp-live.sh

echo "Monitoring HDCP traffic in real-time..."
echo "Showing hex dump on port 53002"
echo "Press Ctrl+C to stop"

sudo tcpdump -i any -nn -XX 'host 192.168.49.1 and port 53002'
