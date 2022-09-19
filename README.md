# Terong

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

### Run

#### As Server

- Find binary for server for Linux at `target/release/terong-server`
or for Windows at `target/release/terong-server.exe`.

<!-- todo(kfj): describe how to set params for tls cert -->

#### As Client

- Find binary for server for Linux at `target/release/terong-client`
or for Windows at `target/release/terong-client.exe`.

<!-- todo(kfj): describe how to set server address -->

<!-- todo(kfj): describe how to set params for tls cert -->
