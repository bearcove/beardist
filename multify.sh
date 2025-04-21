#!/usr/bin/env -S bash -euo pipefail

# Define colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${CYAN}🔍 Starting multi-architecture container manifest creation...${NC}"

# Check if we're on a tag and get the version
TAG_VERSION=""
if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
  TAG_VERSION="${GITHUB_REF#refs/tags/}"
  # Remove 'v' prefix if present
  TAG_VERSION="${TAG_VERSION#v}"
  echo -e "${YELLOW}📦 Detected tag: ${TAG_VERSION}${NC}"
fi

# Define the image tags to use
if [[ -n "$TAG_VERSION" ]]; then
  AMD64_TAG="${TAG_VERSION}-amd64"
  ARM64_TAG="${TAG_VERSION}-arm64"
  # Copy tag-specific AMD64 to latest-amd64
  echo -e "${BLUE}🔄 Copying ${AMD64_TAG} to latest-amd64...${NC}"
  regctl image copy code.bearcove.cloud/bearcove/beardist:${AMD64_TAG} code.bearcove.cloud/bearcove/beardist:latest-amd64
  echo -e "${GREEN}✅ Successfully copied ${AMD64_TAG} to latest-amd64${NC}"

  # Copy tag-specific ARM64 to latest-arm64
  echo -e "${BLUE}🔄 Copying ${ARM64_TAG} to latest-arm64...${NC}"
  regctl image copy code.bearcove.cloud/bearcove/beardist:${ARM64_TAG} code.bearcove.cloud/bearcove/beardist:latest-arm64
  echo -e "${GREEN}✅ Successfully copied ${ARM64_TAG} to latest-arm64${NC}"
  MANIFEST_TAGS=("${TAG_VERSION}" "latest")
else
  AMD64_TAG="latest-amd64"
  ARM64_TAG="latest-arm64"
  MANIFEST_TAGS=("latest")
fi

echo -e "${YELLOW}📦 Getting digests and sizes...${NC}"

echo -e "${BLUE}⬇️  Fetching AMD64 digest...${NC}"
AMD64_DIGEST=$(regctl manifest head code.bearcove.cloud/bearcove/beardist:${AMD64_TAG} --platform linux/amd64)

echo -e "${BLUE}⬇️  Fetching ARM64 digest...${NC}"
ARM64_DIGEST=$(regctl manifest head code.bearcove.cloud/bearcove/beardist:${ARM64_TAG} --platform linux/arm64)

# Check if ARM64_DIGEST is empty or not properly set
if [ -z "$ARM64_DIGEST" ]; then
  echo -e "${RED}❌ Error: Unable to get ARM64 digest. Exiting.${NC}"
  exit 1
else
  echo -e "${GREEN}✅ ARM64 digest retrieved successfully!${NC}"
fi

# Check if AMD64_DIGEST is empty or not properly set
if [ -z "$AMD64_DIGEST" ]; then
  echo -e "${RED}❌ Error: Unable to get AMD64 digest. Exiting.${NC}"
  exit 1
else
  echo -e "${GREEN}✅ AMD64 digest retrieved successfully!${NC}"
fi

echo -e "${BLUE}📏 Calculating AMD64 manifest size...${NC}"
AMD64_SIZE=$(regctl manifest get code.bearcove.cloud/bearcove/beardist:${AMD64_TAG} --platform linux/amd64 --format raw-body | wc -c)
echo -e "${GREEN}✅ AMD64 size: ${AMD64_SIZE} bytes${NC}"

echo -e "${BLUE}📏 Calculating ARM64 manifest size...${NC}"
ARM64_SIZE=$(regctl manifest get code.bearcove.cloud/bearcove/beardist:${ARM64_TAG} --platform linux/arm64 --format raw-body | wc -c)
echo -e "${GREEN}✅ ARM64 size: ${ARM64_SIZE} bytes${NC}"

echo -e "${YELLOW}📝 Creating manifest.json...${NC}"
cat <<EOF > manifest.json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.docker.distribution.manifest.list.v2+json",
  "manifests": [
    {
      "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
      "size": $AMD64_SIZE,
      "digest": "$AMD64_DIGEST",
      "platform": {
        "architecture": "amd64",
        "os": "linux"
      }
    },
    {
      "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
      "size": $ARM64_SIZE,
      "digest": "$ARM64_DIGEST",
      "platform": {
        "architecture": "arm64",
        "os": "linux"
      }
    }
  ]
}
EOF
echo -e "${GREEN}✅ manifest.json created successfully!${NC}"

echo -e "${YELLOW}🚀 Pushing manifest.json to registry...${NC}"
for TAG in "${MANIFEST_TAGS[@]}"; do
  echo -e "${BLUE}📤 Pushing manifest for tag: ${TAG}${NC}"
  regctl manifest put \
    --content-type application/vnd.docker.distribution.manifest.list.v2+json \
    code.bearcove.cloud/bearcove/beardist:${TAG} < manifest.json
  echo -e "${GREEN}✅ Successfully pushed manifest for tag: ${TAG}${NC}"
done
echo -e "${GREEN}🎉 Multi-architecture manifest(s) successfully pushed to registry!${NC}"
