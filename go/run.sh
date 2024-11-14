#!/bin/bash

export GODEBUG=gctrace=1
export GOTRACEBACK=crash

./terong-client
