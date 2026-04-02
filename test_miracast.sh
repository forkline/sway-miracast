#!/bin/bash
set -e

echo "=========================================="
echo "MIRACAST COMPLETE TEST"
echo "=========================================="
echo ""

# Cleanup
echo "Cleaning up..."
pkill -f debug_rtsp 2>/dev/null || true
pkill -f gst-launch 2>/dev/null || true
nmcli connection delete miracast-test 2>/dev/null || true
sleep 2

# Start RTSP server first
echo ""
echo "1. Starting RTSP debug server on port 7236..."
cargo run --example debug_rtsp --package swaybeam-cli 2>&1 &
RTSP_PID=$!
sleep 3

echo "   RTSP server PID: $RTSP_PID"

# Wait for RTSP server to be ready
echo ""
echo "2. Waiting for RTSP server to be ready..."
for i in {1..10}; do
    if ss -tln | grep -q ":7236"; then
        echo "   RTSP server is listening on port 7236"
        break
    fi
    sleep 1
done

# Create P2P connection
echo ""
echo "3. Creating P2P connection profile..."
nmcli connection add type wifi-p2p con-name miracast-test \
    peer "22:28:BC:A8:6C:FE" \
    wifi-p2p.wfd-ies 00000601131C440032 2>&1

echo ""
echo "4. Activating P2P connection..."
echo "   >>> ACCEPT THE CONNECTION ON YOUR TV NOW! <<<"
echo ""

nmcli connection up miracast-test 2>&1 &

# Monitor connection
echo ""
echo "5. Monitoring P2P connection..."
TV_IP=""
OUR_IP=""

for i in {1..60}; do
    STATE=$(nmcli -t -f DEVICE,STATE device status 2>/dev/null | grep p2p-dev | cut -d: -f2)
    
    if [ "$STATE" = "connected" ]; then
        # Get IP addresses
        OUR_IP=$(ip -4 addr show 2>/dev/null | grep -A2 "p2p-wlp" | grep inet | awk '{print $2}' | cut -d/ -f1 | head -1)
        if [ -n "$OUR_IP" ]; then
            TV_IP=$(echo $OUR_IP | sed 's/\.[0-9]*$/.1/')
        fi
        
        if [ -z "$STARTED_STREAM" ] && [ -n "$TV_IP" ]; then
            echo ""
            echo "   P2P CONNECTED!"
            echo "   Our IP: $OUR_IP"
            echo "   TV IP: $TV_IP"
            echo ""
            echo "6. Starting video stream to $TV_IP:5004..."
            
            gst-launch-1.0 \
                videotestsrc pattern=smpte100 ! \
                "video/x-raw,format=I420,width=1920,height=1080,framerate=30/1" ! \
                x264enc tune=zerolatency speed-preset=veryfast bitrate=8000 key-int-max=30 ! \
                "video/x-h264,profile=constrained-baseline,stream-format=byte-stream" ! \
                rtph264pay pt=96 ! \
                udpsink host=$TV_IP port=5004 sync=false async=false 2>&1 &
            
            STREAM_PID=$!
            STARTED_STREAM=1
            echo "   Stream PID: $STREAM_PID"
        fi
    elif [ "$STATE" = "disconnected" ] && [ -n "$STARTED_STREAM" ]; then
        echo ""
        echo "   P2P DISCONNECTED after $i seconds"
        break
    fi
    
    sleep 1
done

echo ""
echo "7. Test complete. Cleaning up..."
kill $RTSP_PID 2>/dev/null || true
kill $STREAM_PID 2>/dev/null || true
nmcli connection delete miracast-test 2>/dev/null || true

echo ""
echo "Done."