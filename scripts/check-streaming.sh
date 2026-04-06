#!/bin/bash
# Check if Miracast streaming is working on LG TV

TV_IP="192.168.49.1"
TV_SSH="root@hall-tv"

echo "=== Miracast Streaming Diagnostics ==="
echo ""

# 1. Check P2P connection
echo "1. P2P Connection:"
ping -c 1 -W 2 $TV_IP > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "   ✓ P2P connection active ($TV_IP)"
else
    echo "   ✗ P2P connection failed"
    exit 1
fi
echo ""

# 2. Check RTSP connection
echo "2. RTSP Connection:"
RTSP_CONN=$(netstat -an 2>/dev/null | grep ":7236.*ESTABLISHED" | grep $TV_IP)
if [ -n "$RTSP_CONN" ]; then
    echo "   ✓ RTSP connection established"
    echo "   $RTSP_CONN"
else
    echo "   ✗ No RTSP connection"
fi
echo ""

# 3. Check UDP stream (local)
echo "3. Local UDP Stream:"
UDP_STREAM=$(ss -u -a 2>/dev/null | grep ":5004")
if [ -n "$UDP_STREAM" ]; then
    echo "   ✓ UDP sink active on port 5004"
    echo "   $UDP_STREAM"
else
    echo "   ✗ No UDP stream detected"
fi
echo ""

# 4. Check TV miracast process
echo "4. TV Miracast Process:"
TV_PROCESS=$(ssh $TV_SSH "ps aux | grep -v grep | grep miracast" 2>/dev/null | head -2)
if [ -n "$TV_PROCESS" ]; then
    echo "   ✓ Miracast process running on TV"
    echo "   $TV_PROCESS"
else
    echo "   ✗ No miracast process on TV"
fi
echo ""

# 5. Check TV network connections
echo "5. TV Network Connections:"
TV_NET=$(ssh $TV_SSH "netstat -an 2>/dev/null | grep 192.168.49" 2>/dev/null | head -5)
if [ -n "$TV_NET" ]; then
    echo "   ✓ TV has network connections to laptop"
    echo "$TV_NET"
else
    echo "   ✗ No TV network connections"
fi
echo ""

# 6. Check RTP packet flow (using tcpdump locally)
echo "6. RTP Packet Flow (5 second capture):"
PACKETS=$(sudo timeout 5 tcpdump -i p2p-wlp2s0-173 -c 100 'udp port 53000 or udp port 5004' 2>/dev/null | grep -c "UDP")
if [ "$PACKETS" -gt 10 ]; then
    echo "   ✓ RTP packets flowing: $PACKETS packets captured"
else
    echo "   ✗ Low/no RTP traffic: $PACKETS packets"
fi
echo ""

# 7. Check GStreamer pipeline
echo "7. GStreamer Pipeline:"
GST_PIPELINE=$(ps aux | grep "swytheam" | grep -v grep)
if [ -n "$GST_PIPELINE" ]; then
    echo "   ✓ swytheam process running"
    echo "   $GST_PIPELINE"
else
    echo "   ✗ No swytheam process"
fi
echo ""

# 8. Check TV framebuffer activity
echo "8. TV Framebuffer Activity:"
TV_FB=$(ssh $TV_SSH "dmesg | tail -20 | grep -i 'framebuffer\|capture'" 2>/dev/null | tail -3)
if [ -n "$TV_FB" ]; then
    echo "   Recent framebuffer activity:"
    echo "$TV_FB"
fi
echo ""

echo "=== Summary ==="
echo "Check the items above. Key indicators:"
echo "- RTSP connection should be ESTABLISHED"
echo "- UDP stream should be active"
echo "- RTP packets should be flowing (tcpdump)"
echo "- TV should have miracast process running"
