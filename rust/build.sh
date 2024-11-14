#!/bin/bash

cargo build --release && \
  cp ./target/release/terong-server . && \
  cp ./target/release/terong-client . && \
  cp ./target/release/rf .
