#!/bin/sh

cargo build --release && \
  cp target/release/duangler-server . && \
  cp target/release/duangler-client .
