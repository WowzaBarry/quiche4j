#!/bin/bash
# Repack the quiche4j-jni JAR with native libraries for all platforms.
# Run after: mvn clean package && ./build-linux-native.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
JNI_DIR="$SCRIPT_DIR/quiche4j-jni"
VERSION="0.4.0"
JAR_BASE="$JNI_DIR/target/quiche4j-jni-${VERSION}"

# Work directory for repacking
WORK="$JNI_DIR/target/repack-work"
rm -rf "$WORK"
mkdir -p "$WORK"

# Extract the existing classifier JAR (has macOS dylib + source + META-INF)
CLASSIFIER_JAR=$(ls "$JAR_BASE"-*.jar 2>/dev/null | grep -v javadoc | head -1)
if [ -z "$CLASSIFIER_JAR" ]; then
    echo "ERROR: No classifier JAR found at $JAR_BASE-*.jar"
    exit 1
fi
echo "Base JAR: $CLASSIFIER_JAR"
cd "$WORK"
jar xf "$CLASSIFIER_JAR"

# Reorganize native-libs into platform subdirectories
# Move existing macOS dylib
if [ -f native-libs/libquiche_jni.dylib ]; then
    mkdir -p native-libs/osx-aarch64
    mv native-libs/libquiche_jni.dylib native-libs/osx-aarch64/
fi

# Copy Linux native libs
LINUX_DIR="$JNI_DIR/target/native-linux"
if [ -d "$LINUX_DIR/linux-x86_64" ]; then
    mkdir -p native-libs/linux-x86_64
    cp "$LINUX_DIR/linux-x86_64/libquiche_jni.so" native-libs/linux-x86_64/
    echo "Added: linux-x86_64"
fi
if [ -d "$LINUX_DIR/linux-aarch64" ]; then
    mkdir -p native-libs/linux-aarch64
    cp "$LINUX_DIR/linux-aarch64/libquiche_jni.so" native-libs/linux-aarch64/
    echo "Added: linux-aarch64"
fi

# Rebuild the JAR (use the plain JAR name, no classifier — for direct deployment)
OUTPUT_JAR="$JNI_DIR/target/quiche4j-jni-${VERSION}.jar"
jar cf "$OUTPUT_JAR" -C "$WORK" .
echo ""
echo "Repacked JAR: $OUTPUT_JAR"
echo "Contents:"
jar tf "$OUTPUT_JAR" | grep native-libs
echo ""
ls -la "$OUTPUT_JAR"
