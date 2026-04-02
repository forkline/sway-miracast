#!/bin/bash
# Install swaybeam

set -e

echo "=== Installing swaybeam ==="

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Detect distro and install dependencies
if command -v pacman &> /dev/null; then
    echo "Detected Arch Linux"
    sudo pacman -S --needed --noconfirm \
        gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly \
        pipewire wireplumber networkmanager wpa_supplicant \
        xdg-desktop-portal xdg-desktop-portal-wlr
elif command -v apt &> /dev/null; then
    echo "Detected Debian/Ubuntu"
    sudo apt update
    sudo apt install -y \
        gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
        gstreamer1.0-plugins-bad gstreamer1.0-libav \
        pipewire wireplumber network-manager wpa_supplicant \
        xdg-desktop-portal-wlr
elif command -v dnf &> /dev/null; then
    echo "Detected Fedora"
    sudo dnf install -y \
        gstreamer1-plugins-base gstreamer1-plugins-good \
        gstreamer1-plugins-bad-free gstreamer1-libav \
        pipewire wireplumber NetworkManager wpa_supplicant \
        xdg-desktop-portal-wlr
else
    echo "Warning: Could not detect package manager"
    echo "Please install: gstreamer, pipewire, networkmanager, wpa_supplicant, xdg-desktop-portal-wlr"
fi

# Build
echo "Building..."
cargo build --release

# Install binary
echo "Installing binary to /usr/local/bin/swaybeam"
sudo cp target/release/swaybeam /usr/local/bin/

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Run 'swaybeam doctor' to check if your system is ready."
echo ""
