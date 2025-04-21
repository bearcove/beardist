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
    libpng16-16 \
    libprotobuf32 \
    tar \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    dav1d \
    && rm -rf /var/lib/apt/lists/*

RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update \
    && apt-get install --no-install-recommends -y \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

RUN curl -LsSf https://astral.sh/uv/install.sh | sh \
    && . $HOME/.local/bin/env \
    && uvx fonttools

ENV PATH="/root/.local/bin:${PATH}"

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
RUN curl -fsSL https://deb.nodesource.com/setup_current.x | bash - \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install --no-install-recommends -y \
    nodejs \
    && rm -rf /var/lib/apt/lists/*
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

####################################################################################################
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

RUN cargo build --release

####################################################################################################
FROM build AS beardist

COPY --from=beardist-builder /build/target/release/beardist /usr/bin/beardist

# Make the binary executable
RUN chmod +x /usr/bin/beardist
