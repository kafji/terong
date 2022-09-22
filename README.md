# Duangler

A [KVM switch software](https://en.wikipedia.org/wiki/KVM_switch).

## Features

- Share mouse and keyboard input between 2 machines.
- Support Linux and Windows.

## Install

### Requirements

- Rust toolchain as described in https://rustup.rs/.

### Build

- Clone this repository.
- Run `cargo build --release`.
- Find binaries for client and server in `target/release` directory.
