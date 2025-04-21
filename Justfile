BASE := "code.bearcove.cloud/bearcove"

check:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo nextest run --no-capture

push:
    #!/usr/bin/env -S bash -euo pipefail
    PLATFORMS=${PLATFORMS:-"linux/arm64 linux/amd64"}
    for target in base build home-base; do \
        echo -e "\033[1;36m🚀 Building target: \033[1;33m$target\033[0m"; \
        for platform in $PLATFORMS; do \
            arch=$(echo $platform | cut -d/ -f2) && \
            echo -e "\033[1;34m📦 Building for platform: \033[0;32m$platform\033[0m" && \
            docker build --platform $platform --tag "{{BASE}}/$target:latest-$arch" --target $target . && \
            echo -e "\033[1;35m⬆️  Pushing image: \033[0;32m{{BASE}}/$target:latest-$arch\033[0m" && \
            docker push "{{BASE}}/$target:latest-$arch"; \
        done && \
        echo -e "\033[1;31m🗑️  Removing existing manifest: \033[0;32m{{BASE}}/$target:latest\033[0m" && \
        docker manifest rm "{{BASE}}/$target:latest" || true && \
        echo -e "\033[1;36m📝 Creating manifest: \033[0;32m{{BASE}}/$target:latest\033[0m" && \
        docker manifest create "{{BASE}}/$target:latest" \
            $(for platform in $PLATFORMS; do \
                arch=$(echo $platform | cut -d/ -f2); \
                echo "{{BASE}}/$target:latest-$arch"; \
            done) && \
        echo -e "\033[1;32m📤 Pushing manifest: \033[0;32m{{BASE}}/$target:latest\033[0m" && \
        docker manifest push "{{BASE}}/$target:latest"; \
        echo -e "\033[1;32m✅ Completed \033[1;33m$target\033[1;32m successfully!\033[0m"; \
    done
