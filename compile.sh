#!/bin/bash

# rust-compile - Dynamic Build script for Cargo projects
# Usage: rust-compile [-a|--android] [-h|--help]

set -e

# Ensure execution from a valid Cargo workspace
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Cargo.toml not found in the current directory."
    exit 1
fi

# Dynamically extract binary name
CARGO_BINARY=$(grep -m 1 -E '^name\s*=' Cargo.toml | awk -F'"' '{print $2}')
if [ -z "$CARGO_BINARY" ]; then
    echo "Error: Could not extract package name from Cargo.toml."
    exit 1
fi

OUTPUT_BINARY="$CARGO_BINARY"
OUTPUT_DIR="$(pwd)"
UPX_AVAILABLE=false

# Check if upx is available
if command -v upx &> /dev/null; then
    UPX_AVAILABLE=true
fi

# Parse arguments
ANDROID=false
while [[ $# -gt 0 ]]; do
    case $1 in
        -a|--android)
            ANDROID=true
            shift
            ;;
        -h|--help)
            echo "Usage: rust-compile [-a|--android] [-h|--help]"
            echo "  -a, --android    Compile for Android (arm64-v8a) API 26"
            echo "  -h, --help       Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "=== Building $OUTPUT_BINARY ==="

if [ "$ANDROID" = true ]; then
    echo ">>> Target: Android (arm64-v8a)"
    
    # Check for Android NDK
    NDK_ROOT="${ANDROID_NDK_ROOT:-$NDK_ROOT}"
    
    # Default to mapped NDK directory if not set globally
    if [ -z "$NDK_ROOT" ]; then
        NDK_ROOT="$HOME/android-ndk-r27c"
    fi
    
    if [ ! -d "$NDK_ROOT" ]; then
        echo "Error: Android NDK not found at $NDK_ROOT"
        exit 1
    fi
    
    # Find Clang
    CLANG="$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang"
    if [ ! -f "$CLANG" ]; then
        CLANG=$(find "$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin" -name "aarch64-linux-android*-clang" 2>/dev/null | head -1)
    fi
    
    if [ -z "$CLANG" ] || [ ! -f "$CLANG" ]; then
        echo "Error: Could not find Android clang"
        exit 1
    fi
    
    # Set target environment
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-L$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/26 -C link-arg=-llog"
    cargo build --release --target aarch64-linux-android
    BINARY_PATH="target/aarch64-linux-android/release/$CARGO_BINARY"
else
    echo ">>> Target: Linux (native)"
    cargo build --release
    BINARY_PATH="target/release/$CARGO_BINARY"
fi

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Build failed - binary not found at $BINARY_PATH"
    exit 1
fi

echo ">>> Stripping binary..."
if [ "$ANDROID" = true ]; then
    STRIP_BIN="$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
    if [ -f "$STRIP_BIN" ]; then
        "$STRIP_BIN" "$BINARY_PATH"
    else
        strip "$BINARY_PATH"
    fi
else
    strip "$BINARY_PATH"
fi

if [ "$UPX_AVAILABLE" = true ]; then
    echo ">>> Compressing with UPX..."
    upx --ultra-brute "$BINARY_PATH" || echo "Warning: UPX compression failed"
else
    echo "Warning: upx not found, skipping compression"
fi

echo ">>> Moving to $OUTPUT_DIR..."
cp "$BINARY_PATH" "$OUTPUT_DIR/$OUTPUT_BINARY"
chmod +x "$OUTPUT_DIR/$OUTPUT_BINARY"

# Get final size
FINAL_SIZE=$(stat -c%s "$OUTPUT_DIR/$OUTPUT_BINARY")
echo ">>> Done! Final size: $((FINAL_SIZE / 1024)) KB"