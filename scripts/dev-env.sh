#!/bin/bash
set -e

echo "Setting up development environment..."

sudo apt install -y \
    build-essential \
    rustc cargo \
    libpipewire-0.3-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-libav \
    network-manager \
    wpasupplicant

echo "Done!"