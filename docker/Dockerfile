# SPDX-License-Identifier: MIT
# Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
#
# Everything needed to build and check CPOS: the ARM toolchain, QEMU 7.2,
# stable Rust with the Cortex-M3 target, and nightly with Miri and
# cargo-fuzz. CI uses this image; so can you:
#
#   docker build -t cpos-dev docker
#   docker run --rm -it -v "$PWD:/src" -w /src cpos-dev bash

FROM rust:bookworm

RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		gcc-arm-none-eabi libnewlib-arm-none-eabi qemu-system-arm make python3 \
	&& rm -rf /var/lib/apt/lists/*

RUN rustup target add thumbv7m-none-eabi \
	&& rustup toolchain install nightly --profile minimal --component miri,rust-src \
	&& cargo +nightly miri setup

RUN cargo install cargo-fuzz --locked
