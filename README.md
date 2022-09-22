# Duangler

A [KVM switch software](https://en.wikipedia.org/wiki/KVM_switch).

## Features

- Share mouse and keyboard input between 2 machines.
- Support Linux and Windows.
- Switch active machine by double tapping the right ctrl key.

## Installation

### Requirements

- Rust toolchain as described in https://rustup.rs/.

### Build

- Clone this repository.
- Run `cargo build --release`.
- Find binaries in `target/release` directory.

### Install

#### Server

<details>

<summary>Linux</summary>

- Copy the server binary `duangler-server` into a directory.

- Copy config file example [example.duangler.toml](example.duangler.toml) into the same directory.

- Rename config file example to `duangler.toml`.

- Replace values in the server section with your configuration. e.g.

```toml
[server]
# Where the server will listen for incoming connections.
port = 3000
# This server IP address.
# This field is used to generate TLS key at runtime.
addr = "192.168.0.1"
```

- Run server in sudo mode e.g.

```bash
sudo ./duangler-server
```

</details>

<details>

<summary>Windows</summary>

- Copy the server binary `duangler-server.exe` into a directory.

- Copy config file example [example.duangler.toml](example.duangler.toml) into the same directory.

- Rename config file example to `duangler.toml`.

- Replace values in the server section with your configuration. e.g.

```toml
[server]
# Where the server will listen for incoming connections.
port = 3000
# This server IP address.
# This field is used to generate TLS key at runtime.
addr = "192.168.0.1"
```

- Run server by double clicking `duangler-server.exe`.

</details>

#### Client

<details>

<summary>Linux</summary>

- Copy the client binary `duangler-client` into a directory.

- Copy config file example [example.duangler.toml](example.duangler.toml) into the same directory.

- Rename config file example to `duangler.toml`.

- Replace values in the client section with your configuration. e.g.

```toml
[client]
# This client IP address.
# This field is used to generate TLS key at runtime.
addr = "192.168.0.2"
# Address of the server the client will connect to.
server_addr = "192.168.0.1:3000"
```

- Run client in sudo mode e.g.

```bash
sudo ./duangler-client
```

</details>

<details>

<summary>Windows</summary>

- Copy the server binary `duangler-server.exe` into a directory.

- Copy config file example [example.duangler.toml](example.duangler.toml) into the same directory.

- Rename config file example to `duangler.toml`.

- Replace values in the client section with your configuration. e.g.

```toml
[server]
# Where the server will listen for incoming connections.
port = 3000
# This server IP address.
# This field is used to generate TLS key at runtime.
addr = "192.168.0.1"
```

- Run server by double clicking `duangler-server.exe`.

<details>
