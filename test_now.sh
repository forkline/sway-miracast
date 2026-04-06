#!/bin/bash
set -e

echo "=== MIRACAST TEST $(date +%H:%M:%S) ==="

# Cleanup
pkill -f debug_rtsp 2>/dev/null || true
pkill -f gst-launch 2>/dev/null || true
nmcli con del test 2>/dev/null || true
sleep 2

# Start RTSP
echo "1. Starting RTSP server..."
cargo run --example debug_rtsp --package swaybeam-cli 2>&1 &
RTSP_PID=$!
sleep 3

# Verify RTSP
if ss -tln | grep -q 7236; then
    echo "   ✓ RTSP listening on 7236 (PID: $RTSP_PID)"
else
    echo "   ✗ RTSP not listening"
    exit 1
fi

# Create P2P
echo ""
echo "2. Creating P2P connection..."
nmcli con add type wifi-p2p con-name test \
    peer "22:28:BC:A8:6C:FE" \
    wifi-p2p.wfd-ies 00000600901C4400C8 2>&1 | grep -v "^Connection"

# Activate
echo ""
echo "3. Activating P2P..."
echo "   >>> ACCEPT ON YOUR TV NOW! <<<"
nmcli con up test 2>&1 &

# Monitor
echo ""
echo "4. Monitoring..."
for i in $(seq 1 45); do
    STATE=$(nmcli -t -f DEVICE,STATE dev status 2>/dev/null | grep p2p-dev | cut -d: -f2)

    if [ "$STATE" = "connected" ]; then
        OUR_IP=$(ip -4 addr show 2>/dev/null | grep -A2 "p2p-wlp" | grep inet | awk '{print $2}' | cut -d/ -f1 | head -1)
        TV_IP=$(echo $OUR_IP | sed 's/\.[0-9]*$/.1/')

        echo ""
        echo "=========================================="
        echo "[$i] CONNECTED! $OUR_IP -> TV: $TV_IP"
        echo "=========================================="

        if [ -z "$STREAM_STARTED" ]; then
            echo "Starting video stream to $TV_IP..."
            gst-launch-1.0 videotestsrc pattern=smpte100 \
                ! video/x-raw,format=I420,width=1920,height=1080,framerate=30/1 \
                ! x264enc tune=zerolatency speed-preset=veryfast bitrate=8000 \
                ! video/x-h264,profile=constrained-baseline,stream-format=byte-stream \
                ! rtph264pay pt=96 \
                ! udpsink host=$TV_IP port=5004 sync=false async=false 2>/dev/null &
            STREAM_STARTED=1
            echo ""
            echo "=========================================="
            echo "CHECK YOUR TV - COLOR BARS!"
            echo "=========================================="
        fi
    elif [ "$STATE" = "disconnected" ] && [ -n "$STREAM_STARTED" ]; then
        echo ""
        echo "[$i] Disconnected"
        break
    else
        printf "\r[%02d] %-30s" "$i" "$STATE"
    fi
    sleep 1
done

echo ""
echo ""
echo "5. Logs..."
echo ""
echo "=== Laptop wpa_supplicant ==="
journalctl -u wpa_supplicant --since "2 minutes ago" 2>&1 | grep -E "P2P-GO|P2P-GROUP|success|FORMATION" | tail -10

echo ""
echo "=== TV kernel ==="
ssh root@hall-tv 'dmesg | grep -i p2p | tail -10' 2>&1

echo ""
echo "6. Cleanup..."
pkill -f debug_rtsp 2>/dev/null || true
pkill -f gst-launch 2>/dev/null || true
nmcli con del test 2>/dev/null || true

echo ""
echo "=== TEST COMPLETE ==="
