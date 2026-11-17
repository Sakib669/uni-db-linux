# GTK build environment for the native app, without a bundled Rust toolchain.
# The host Rust toolchain and cargo registry are mounted at run time, so the
# image only carries the GTK development packages.
# Ubuntu 24.04 ships GTK 4.14 and libadwaita 1.5, the floor from spec 0001.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    git \
    pkg-config \
    libgtk-4-dev \
    libadwaita-1-dev \
    libgtksourceview-5-dev \
    libsecret-1-dev \
    xvfb \
    xdotool \
    imagemagick \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
