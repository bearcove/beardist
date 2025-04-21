# syntax=docker/dockerfile:1

####################################################################################################
FROM ubuntu:24.04 AS base

RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    bash \
    bsdmainutils \
    bzip2 \
    ca-certificates \
    coreutils \
    curl \
    gzip \
    libcurl4 \
    dav1d \
    libpng16-16 \
    libprotobuf32 \
    sqlite3 \
    tar \
    fonttools \
    xz-utils \
    && curl -fsSL https://deb.nodesource.com/setup_current.x | bash - \
    && apt-get install --no-install-recommends -y nodejs \
    && rm -rf /var/lib/apt/lists/*

####################################################################################################
FROM base AS build
RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    clang \
    && rm -rf /var/lib/apt/lists/*
RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    autoconf \
    autotools-dev \
    cmake \
    git \
    jq \
    libtool \
    libtool-bin \
    make \
    nasm \
    ninja-build \
    openssh-client \
    patch \
    unzip \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    libcurl4-openssl-dev \
    libdav1d-dev \
    libgtest-dev \
    libpng-dev \
    libprotobuf-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"
RUN npm install -g pnpm

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
ENV PATH="/root/.cargo/bin:${PATH}" \
    CARGO_PROFILE_RELEASE_DEBUG="line-tables-only" \
    CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO="packed" \
    CC=clang \
    CXX=clang++
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo-binstall -y cargo-sweep cargo-nextest just cargo-chef

RUN set -eux; \
    arch=$([ "$(uname -m)" = "aarch64" ] && echo "arm64" || echo "amd64") && \
    curl -L https://github.com/regclient/regclient/releases/latest/download/regctl-linux-${arch} > /usr/bin/regctl \
    && chmod 755 /usr/bin/regctl \
    && regctl version

RUN set -eux; \
    npm install -g esbuild

####################################################################################################
FROM base AS home-base

RUN set -eux; \
    export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    imagemagick \
    iproute2 \
    iputils-ping \
    dnsutils \
    curl \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    echo "Checking for required tools..." && \
    which curl || (echo "curl not found" && exit 1) && \
    which tar || (echo "tar not found" && exit 1) && \
    which ip || (echo "ip not found" && exit 1) && \
    which ping || (echo "ping not found" && exit 1) && \
    which dig || (echo "dig not found" && exit 1) && \
    which nslookup || (echo "nslookup not found" && exit 1) && \
    echo "Creating FFmpeg directory..." && \
    mkdir -p /opt/ffmpeg && \
    echo "Downloading FFmpeg..." && \
    arch=$([ "$(uname -m)" = "aarch64" ] && echo "linuxarm64" || echo "linux64") && \
    echo "Downloading $arch build" && \
    curl -sSL --retry 3 --retry-delay 3 \
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-${arch}-gpl-shared.tar.xz" -o /tmp/ffmpeg.tar.xz && \
    echo "Extracting FFmpeg..." && \
    tar -xJf /tmp/ffmpeg.tar.xz --strip-components=1 -C /opt/ffmpeg && \
    rm -f /tmp/ffmpeg.tar.xz
ENV \
    FFMPEG=/opt/ffmpeg \
    PATH=$PATH:/opt/ffmpeg/bin \
    LD_LIBRARY_PATH=/opt/ffmpeg/lib
RUN set -eux; \
    echo "Verifying FFmpeg installation..." && \
    ffmpeg -version || (echo "FFmpeg installation failed" && exit 1) && \
    echo "FFmpeg installation successful"

# apparently `libsqlite3.so` is only installed by the `-dev` package, but our program relies on it, so...
RUN arch=$([ "$(uname -m)" = "aarch64" ] && echo "aarch64" || echo "x86_64") \
    && ln -s "/usr/lib/${arch}-linux-gnu/libsqlite3.so.0" "/usr/lib/${arch}-linux-gnu/libsqlite3.so"

# Define a Docker Buildx Bake configuration
FROM build AS beardist-builder

WORKDIR /build

COPY src/ ./src/
COPY Cargo.toml .

RUN rustup default stable

RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY recipe.json .
RUN cargo chef cook --recipe-path recipe.json

RUN cargo build --release

####################################################################################################
FROM build AS beardist

COPY --from=beardist-builder /build/target/release/beardist /usr/bin/beardist

# Make the binary executable
RUN chmod +x /usr/bin/beardist
