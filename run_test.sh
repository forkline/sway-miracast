#!/bin/bash
set -x

echo "=== MIRACAST TEST ==="

# Cleanup
pkill -f debug_rtsp 2>/dev/null || true
pkill -f gst-launch 2>/dev/null || true
nmcli con del miracast-test 2>/dev/null || true
sleep 1

# Start RTSP
echo "Starting RTSP..."
cargo run --example debug_rtsp --package swaybeam-cli &
RTSP_PID=$!
sleep 3

ss -tln | grep 7236

# Create P2P
echo "Creating P2P..."
nmcli con add type wifi-p2p con-name miracast-test peer "22:28:BC:A8:6C:FE" wifi-p2p.wfd-ies 000006051C4400C800

# Activate
echo "Activating P2P..."
nmcli con up miracast-test &
P2P_PID=$!

# Monitor
echo "Monitoring..."
for i in {1..50}; do
    STATE=$(nmcli -t -f DEVICE,STATE dev status 2>/dev/null | grep p2p-dev | cut -d: -f2)
    echo "[$i] State: $STATE"
    
    if [ "$STATE" = "connected" ]; then
        OUR_IP=$(ip -4 addr show 2>/dev/null | grep -A2 "p2p-wlp" | grep inet | awk '{print $2}' | cut -d/ -f1 | head -1)
        TV_IP=$(echo $OUR_IP | sed 's/\.[0-9]*$/.1/')
        echo "CONNECTED! IP: $OUR_IP TV: $TV_IP"
        
        if [ -z "$STREAM_STARTED" ]; then
            echo "Starting stream to $TV_IP..."
            gst-launch-1.0 videotestsrc pattern=smpte100 ! "video/x-raw,format=I420,width=1920,height=1080,framerate=30/1" ! x264enc tune=zerolatency speed-preset=veryfast bitrate=8000 ! "video/x-h264,profile=constrained-baseline,stream-format=byte-stream" ! rtph264pay pt=96 ! udpsink host=$TV_IP port=5004 sync=false async=false &
            STREAM_STARTED=1
        fi
    fi
    
    sleep 1
done

echo "Cleanup..."
pkill -f debug_rtsp 2>/dev/null
pkill -f gst-launch 2>/dev/null
nmcli con del miracast-test 2>/dev/null
echo "Done"