# Base image: Ubuntu 22.04 LTS (best compatibility with cnb.cool)
FROM ubuntu:22.04

# Core environment variables
ENV DEBIAN_FRONTEND=noninteractive
ENV QEMU_VERSION=10.2.0
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
# Fixed nightly version (2025-12-12)
ENV RUST_NIGHTLY_FIXED=2025-12-12
# Global default version: latest stable
ENV RUST_DEFAULT_TOOLCHAIN=stable

# Step 1: Install base dependencies
RUN apt update && apt install -y \
    # QEMU build dependencies
    build-essential git wget curl ninja-build pkg-config \
    libglib2.0-dev libfdt-dev libpixman-1-dev zlib1g-dev \
    libssl-dev libncurses5-dev flex bison libnuma-dev \
    libseccomp-dev libcapstone-dev libvde-dev libslirp-dev \
    # Rust cross-compilation dependencies
    gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
    gcc-riscv64-linux-gnu g++-riscv64-linux-gnu \
    # Auxiliary tools
    qemu-user-static binfmt-support \
    python3-venv python3-full python3-dev python3-pip python3-tomli python3-tomli-w \
    && rm -rf /var/lib/apt/lists/*

# Step 2: Compile and install QEMU 10.2.0 (all target architectures)
RUN wget https://github.com/qemu/qemu/archive/refs/tags/v${QEMU_VERSION}.tar.gz -O /tmp/qemu.tar.gz \
    && tar -xf /tmp/qemu.tar.gz -C /tmp && rm /tmp/qemu.tar.gz \
    && cd /tmp/qemu-${QEMU_VERSION} \
    && ./configure \
        --prefix=/usr/local \
        --target-list=x86_64-softmmu,aarch64-softmmu,loongarch64-softmmu,riscv64-softmmu \
        --disable-werror --disable-docs --disable-vnc --disable-sdl \
        --enable-gcov --enable-debug --enable-slirp \
    && make -j"$(nproc)" && make install \
    && rm -rf /tmp/qemu-${QEMU_VERSION}

# Step 3: Install Rust (latest stable as default + fixed version + latest nightly)
# Accelerate Rust download (China mirror, optional)
ENV RUSTUP_DIST_SERVER=https://rsproxy.cn
ENV RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup.sh \
    && chmod +x /tmp/rustup.sh \
    && /tmp/rustup.sh -y --default-toolchain ${RUST_DEFAULT_TOOLCHAIN} --no-modify-path \
    && rm /tmp/rustup.sh \
    # Install fixed version (2025-12-12)
    && rustup install nightly-${RUST_NIGHTLY_FIXED} \
    # Install latest nightly version
    && rustup install nightly \
    # Add multi-architecture cross-compilation targets for all versions
    && for toolchain in \
        stable \
        nightly-${RUST_NIGHTLY_FIXED} \
        nightly; do \
        rustup target add \
            aarch64-unknown-linux-gnu \
            loongarch64-unknown-linux-gnu \
            riscv64gc-unknown-linux-gnu \
            x86_64-unknown-linux-gnu \
            aarch64-unknown-none-softfloat \
            loongarch64-unknown-none \
            loongarch64-unknown-none-softfloat \
            riscv64gc-unknown-none-elf \
            x86_64-unknown-none \
            --toolchain ${toolchain}; \
    done \
    # Verify installation (output version info)
    && echo "=== Default (latest stable) version ===" && rustc --version \
    && echo "=== Fixed nightly version ===" && rustc +nightly-${RUST_NIGHTLY_FIXED} --version \
    && echo "=== Latest nightly version ===" && rustc +nightly --version

# Step 4: Configure Rust cross-compilation linkers (global)
RUN mkdir -p /root/.cargo \
    # && echo '[target.aarch64-unknown-linux-gnu]' > /root/.cargo/config \
    # && echo 'linker = "aarch64-linux-gnu-gcc"' >> /root/.cargo/config \
    # && echo '[target.riscv64gc-unknown-linux-gnu]' >> /root/.cargo/config \
    # && echo 'linker = "riscv64-linux-gnu-gcc"' >> /root/.cargo/config \
    # && echo '[target.loongarch64-unknown-linux-gnu]' >> /root/.cargo/config \
    # && echo 'linker = "loongarch64-linux-gnu-gcc"' >> /root/.cargo/config \
    && echo '# Rust version switch aliases' >> /root/.bashrc \
    && echo 'alias rust-use-default="rustup default stable"' >> /root/.bashrc \
    && echo 'alias rust-use-fixed-nightly="rustup default nightly-2025-12-12"' >> /root/.bashrc \
    && echo 'alias rust-use-latest-nightly="rustup default nightly"' >> /root/.bashrc \
    && echo 'alias rust-list-all="rustup toolchain list"' >> /root/.bashrc

# Step 5: Download musl cross-compilation toolchains
RUN cd / && \
    wget https://musl.cc/aarch64-linux-musl-cross.tgz && \
    wget https://musl.cc/riscv64-linux-musl-cross.tgz && \
    wget https://musl.cc/x86_64-linux-musl-cross.tgz && \
    wget https://github.com/LoongsonLab/oscomp-toolchains-for-oskernel/releases/download/loongarch64-linux-musl-cross-gcc-13.2.0/loongarch64-linux-musl-cross.tgz && \
    tar zxf aarch64-linux-musl-cross.tgz && \
    tar zxf riscv64-linux-musl-cross.tgz && \
    tar zxf x86_64-linux-musl-cross.tgz && \
    tar zxf loongarch64-linux-musl-cross.tgz && \
    rm -f *.tgz

ENV PATH="/x86_64-linux-musl-cross/bin:/aarch64-linux-musl-cross/bin:/riscv64-linux-musl-cross/bin:/loongarch64-linux-musl-cross/bin:$PATH"

# Step 6: Install cargo auxiliary tools
RUN cargo install cargo-binutils cargo-clone

# Verify all components
RUN qemu-system-x86_64 --version \
    && qemu-system-aarch64 --version \
    && qemu-system-loongarch64 --version \
    && qemu-system-riscv64 --version \
    && rustup target list --toolchain stable | grep "installed"

# Default command (adapted for cnb.cool interactive environment)
CMD ["/bin/bash"]
