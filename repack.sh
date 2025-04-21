#!/usr/bin/env bash
set -euo pipefail

# This script creates a container image for beardist
# The process involves:
#  1. Creating an OCI layout directory for beardist
#  2. Adding beardist to the image
#  3. Pushing the final image with proper tags

if [ "${IMAGE_PLATFORM}" != "linux/arm64" ] && [ "${IMAGE_PLATFORM}" != "linux/amd64" ]; then
    echo -e "\033[1;31m❌ Error: IMAGE_PLATFORM must be set to linux/arm64 or linux/amd64\033[0m" >&2
    exit 1
fi

ARCH_NAME=$(echo "${IMAGE_PLATFORM}" | cut -d'/' -f2)

# Check if we're on a tag and get the version
TAG_VERSION=""
if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
  TAG_VERSION="${GITHUB_REF#refs/tags/}"
  # Remove 'v' prefix if present
  TAG_VERSION="${TAG_VERSION#v}"
  echo -e "\033[1;33m📦 Detected tag: ${TAG_VERSION}\033[0m"
fi

# Declare variables
OCI_LAYOUT_DIR="/tmp/beardist-oci-layout"
OUTPUT_DIR="/tmp/beardist-output"
IMAGE_NAME="code.bearcove.cloud/bearcove/beardist:${TAG_VERSION:+${TAG_VERSION}-}${ARCH_NAME}"
BASE_IMAGE="code.bearcove.cloud/bearcove/build:${ARCH_NAME}"

# Clean up and create layout directory
rm -rf "$OCI_LAYOUT_DIR"
mkdir -p "$OCI_LAYOUT_DIR/usr/bin"

# Copy beardist to the layout directory
echo -e "\033[1;34m📦 Copying beardist binary to layout directory\033[0m"
cp -v "$OUTPUT_DIR/beardist" "$OCI_LAYOUT_DIR/usr/bin/"

# Reset all timestamps to epoch for reproducible builds
touch -t 197001010000.00 "$OCI_LAYOUT_DIR/usr/bin/beardist"

# Create the image
echo -e "\033[1;36m🔄 Creating image from base\033[0m"
regctl image mod "$BASE_IMAGE" --create "$IMAGE_NAME" \
    --layer-add "dir=$OCI_LAYOUT_DIR"

# Push the image
echo -e "\033[1;32m🚀 Pushing image: \033[1;35m$IMAGE_NAME\033[0m"
regctl image copy "$IMAGE_NAME"{,}

# Push tagged image if we're in CI and there's a tag
if [ -n "${CI:-}" ] && [ -n "${GITHUB_REF:-}" ]; then
    if [[ "$GITHUB_REF" == refs/tags/* ]]; then
        TAG=${GITHUB_REF#refs/tags/}
        if [[ "$TAG" == v* ]]; then
            TAG=${TAG#v}
        fi
        TAGGED_IMAGE_NAME="code.bearcove.cloud/bearcove/beardist:$TAG"
        echo -e "\033[1;32m🏷️ Tagging and pushing: \033[1;35m$TAGGED_IMAGE_NAME\033[0m"
        regctl image copy "$IMAGE_NAME" "$TAGGED_IMAGE_NAME"
    fi
fi

# Test the image if not in CI
if [ -z "${CI:-}" ]; then
    echo -e "\033[1;34m🧪 Testing image locally\033[0m"
    docker pull "$IMAGE_NAME"
    docker run --rm "$IMAGE_NAME" beardist --help
    
    # Display image info
    echo -e "\033[1;35m📋 Image layer information:\033[0m"
    docker image inspect "$IMAGE_NAME" --format '{{.RootFS.Layers | len}} layers'
fi