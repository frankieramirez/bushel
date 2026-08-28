#!/bin/sh
set -eu
container image pull nginx:alpine
container image pull redis:alpine
container image pull node:22-slim
container run -d --name web nginx:alpine
container run -d --name cache redis:alpine
container run -d --name worker node:22-slim sh -c \
  'i=0; while true; do i=$((i+1)); echo "$(date -u +%H:%M:%S) worker: processed job #$i"; sleep 2; done'
