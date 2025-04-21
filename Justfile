BASE := "ghcr.io/bearcove"

check:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo nextest run --no-capture

push:
    #!/usr/bin/env -S bash -euxo pipefail
    PLATFORMS=${PLATFORMS:-"linux/arm64,linux/amd64"}

    # Define tags array
    tags=()

    # Check if we're on a tag and get the version
    if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
        TAG_VERSION="${GITHUB_REF#refs/tags/}"
        # Remove 'v' prefix if present
        TAG_VERSION="${TAG_VERSION#v}"
        echo -e "\033[1;33m📦 Detected tag: ${TAG_VERSION}\033[0m"

        # Add version-specific tags
        tags+=("--tag" "{{BASE}}/beardist:${TAG_VERSION}")
    fi

    # Add latest tag
    tags+=("--tag" "{{BASE}}/beardist:latest")

    # Build for all platforms at once
    docker buildx build \
        --target beardist \
        --platform "${PLATFORMS}" \
        "${tags[@]}" \
        --push \
        .
