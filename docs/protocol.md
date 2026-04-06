# Protocol

## Wi-Fi Direct / P2P

Miracast uses Wi-Fi Direct (Wi-Fi P2P) for device discovery and connection.

## RTSP / WFD

Wi-Fi Display (WFD) protocol runs over RTSP on port 7236.

### Message Flow

1. **OPTIONS** - Capability exchange
2. **GET_PARAMETER** - Parameter negotiation
3. **SET_PARAMETER** - WFD parameters
4. **PLAY** - Start streaming
5. **TEARDOWN** - End session

## Media

- H.264 codec for video
- AAC or LPCM for audio
- RTP transport
