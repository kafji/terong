# Duangler

A [KVM switch software](https://en.wikipedia.org/wiki/KVM_switch).

## Features

- Share mouse and keyboard input between 2 machines.
- Support Windows and Linux (server for Linux is work in progress).
- Switch active machine by double tapping the right ctrl key.

## Installation

### Requirements

- Rust toolchain as described in https://rustup.rs/.

### Install

- Clone repository.
- Run `cargo build --release`.
- Find binaries in `./target/release`.
