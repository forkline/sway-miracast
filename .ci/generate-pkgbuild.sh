#!/bin/bash
set -e

PKG_TYPE="${1:-PKGBUILD}"
VERSION="${PROJECT_VERSION:-0.1.0}"

case "$PKG_TYPE" in
    PKGBUILD)
        cat > PKGBUILD << EOF
# Maintainer: Pando85 <pando855@gmail.com>
pkgname=swaybeam
pkgver=${VERSION}
pkgrel=1
pkgdesc="Miracast source implementation for wlroots-based compositors written in Rust"
arch=('x86_64')
url="https://github.com/forkline/swaybeam"
license=('MIT')
depends=('glibc' 'gstreamer' 'gst-plugins-base' 'gst-plugins-good' 'gst-plugins-bad' 'pipewire')
makedepends=('cargo' 'rust' 'git')

source=("git+https://github.com/forkline/swaybeam.git#tag=v${pkgver}")
sha256sums=('SKIP')

pkgver() {
    cd swaybeam
    echo $(grep "^version" Cargo.toml | head -n1 | sed 's/version = "//' | sed 's/"//')
}

build() {
    cd swaybeam
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --release --locked
}

package() {
    cd swaybeam
    install -Dm755 "target/release/swaybeam" "\${pkgdir}/usr/bin/swaybeam"
}
EOF
        ;;
    PKGBUILD-bin)
        SHA256=$(curl -sL "https://github.com/forkline/swaybeam/releases/download/v${VERSION}/swaybeam-${VERSION}-x86_64.tar.gz.sha256" | cut -d ' ' -f 1)
        cat > PKGBUILD-bin << EOF
# Maintainer: Pando85 <pando855@gmail.com>
pkgname=swaybeam-bin
pkgver=${VERSION}
pkgrel=1
pkgdesc="Miracast source implementation for wlroots-based compositors written in Rust (binary)"
arch=('x86_64')
url="https://github.com/forkline/swaybeam"
license=('MIT')
provides=('swaybeam')
conflicts=('swaybeam')

source_x86_64=("swaybeam-\${pkgver}-x86_64.tar.gz::https://github.com/forkline/swaybeam/releases/download/v\${pkgver}/swaybeam-\${pkgver}-x86_64.tar.gz")
sha256sums_x86_64=('${SHA256}')

package() {
    tar -xzf swaybeam-\${pkgver}-x86_64.tar.gz
    install -Dm755 "swaybeam" "\${pkgdir}/usr/bin/swaybeam"
}
EOF
        ;;
    *)
        echo "Unknown PKG_TYPE: $PKG_TYPE"
        exit 1
        ;;
esac

echo "Generated $PKG_TYPE for version ${VERSION}"
