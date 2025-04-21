BASE := "ghcr.io/bearcove"

check:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo nextest run --no-capture

push:
    #!/usr/bin/env -S bash -euo pipefail
    PLATFORMS=${PLATFORMS:-"linux/arm64 linux/amd64"}
    for platform in $PLATFORMS; do \
        arch=$(echo $platform | cut -d'/' -f2)
        tag="{{BASE}}/beardist:${arch}-latest"

        # Check if we're on a tag and get the version
        if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
            TAG_VERSION="${GITHUB_REF#refs/tags/}"
            # Remove 'v' prefix if present
            TAG_VERSION="${TAG_VERSION#v}"
            echo -e "\033[1;33m📦 Detected tag: ${TAG_VERSION}\033[0m"
            # Add version-specific tag
            tag="{{BASE}}/beardist:${arch}-${TAG_VERSION}"
        fi

        docker buildx build \
            --platform "${platform}" \
            --tag ${tag} \
            beardist
    done
