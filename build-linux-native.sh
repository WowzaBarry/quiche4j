#!/bin/bash
# Build quiche4j JNI native libraries for Linux (x86_64 and aarch64) using Docker.
# Uses native ARM64 container with cross-compilation for x86_64 (avoids QEMU crashes).
# Produces .so files in quiche4j-jni/target/native-linux/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
JNI_DIR="$SCRIPT_DIR/quiche4j-jni"
OUT_DIR="$JNI_DIR/target/native-linux"
mkdir -p "$OUT_DIR"

RUST_IMAGE="rust:1.85-bookworm"

QUICHE_DIR="${QUICHE_DIR:-$(cd "$SCRIPT_DIR/../quiche" && pwd)}"
echo "Using quiche source: $QUICHE_DIR"

echo "=== Building for linux-aarch64 (native) ==="
docker run --rm --platform linux/arm64 \
    -v "$SCRIPT_DIR:/src" \
    -v "$QUICHE_DIR:/quiche" \
    -w /src/quiche4j-jni \
    "${RUST_IMAGE}" \
    bash -c "
        apt-get update -qq && apt-get install -y -qq cmake >/dev/null 2>&1
        # Rewrite path dependency for Docker environment
        sed -i 's|path = \".*quiche.*\"|path = \"/quiche/quiche\"|' Cargo.toml
        cargo build --lib --release --target-dir /tmp/cargo-target
        mkdir -p /src/quiche4j-jni/target/native-linux/linux-aarch64
        cp /tmp/cargo-target/release/libquiche_jni.so /src/quiche4j-jni/target/native-linux/linux-aarch64/
    "
echo "=== Done: linux-aarch64 ==="
ls -la "$OUT_DIR/linux-aarch64/"

echo ""
echo "=== Building for linux-x86_64 (cross-compile from ARM64) ==="
docker run --rm --platform linux/arm64 \
    -v "$SCRIPT_DIR:/src" \
    -v "$QUICHE_DIR:/quiche" \
    -w /src/quiche4j-jni \
    "${RUST_IMAGE}" \
    bash -c "
        apt-get update -qq && apt-get install -y -qq cmake gcc-x86-64-linux-gnu g++-x86-64-linux-gnu >/dev/null 2>&1
        # Rewrite path dependency for Docker environment
        sed -i 's|path = \".*quiche.*\"|path = \"/quiche/quiche\"|' Cargo.toml
        rustup target add x86_64-unknown-linux-gnu
        export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
        export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
        export CXX_x86_64_unknown_linux_gnu=x86_64-linux-gnu-g++
        cargo build --lib --release --target x86_64-unknown-linux-gnu --target-dir /tmp/cargo-target
        mkdir -p /src/quiche4j-jni/target/native-linux/linux-x86_64
        cp /tmp/cargo-target/x86_64-unknown-linux-gnu/release/libquiche_jni.so /src/quiche4j-jni/target/native-linux/linux-x86_64/
    "
echo "=== Done: linux-x86_64 ==="
ls -la "$OUT_DIR/linux-x86_64/"

echo ""
echo "Linux native libraries built:"
find "$OUT_DIR" -type f -name "*.so" -exec ls -la {} \;
