#!/bin/bash

cargo build --release && \
  cp target/release/duangler-server . && \
  cp target/release/duangler-client .
