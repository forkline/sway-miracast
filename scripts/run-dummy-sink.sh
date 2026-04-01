#!/bin/bash
# Run a dummy Miracast sink for testing
# Requires: gst-rtsp-launch

gst-rtsp-launch -p 8554 \
    "( videotestsrc ! x264enc ! rtph264pay name=pay0 pt=96 )"