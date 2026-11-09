#!/usr/bin/env bash
# Build a FlowState .rpm on Fedora. Run from anywhere; output lands in rust/target/generate-rpm/.
set -euo pipefail
cd "$(dirname "$0")/../.."

sudo dnf install -y gcc gcc-c++ cmake clang clang-devel pkgconf-pkg-config \
    alsa-lib-devel libxkbcommon-devel wayland-devel libX11-devel \
    libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel

command -v cargo >/dev/null || { echo "Install Rust first: https://rustup.rs"; exit 1; }
command -v cargo-generate-rpm >/dev/null || cargo install cargo-generate-rpm --locked

cargo build --release
cargo generate-rpm

echo
echo "Install with:  sudo dnf install $(ls target/generate-rpm/*.rpm | head -n1)"
