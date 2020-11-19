#!/bin/bash

set -eEuo pipefail

mkdir -p /root/workspace
cd /root/workspace

rsync -av \
  --exclude 'target/' \
  --exclude '.cargo/' \
  --exclude '.git/' \
  --exclude '.wasm_cache/' \
  --exclude '.wasm_target/' \
  --exclude '.vagrant/' \
  --exclude 'containernet/' \
  /holochain/ /root/workspace/

docker run -v /root/workspace:/work \
  --env CARGO_TARGET_DIR=/work/target \
  --env CARGO_HOME=/work/.cargo \
  rust:1.47-slim-buster \
  bash -c 'cd /work; cargo build --release --bin kitsune-p2p-proxy'

docker run -v /root/workspace:/work \
  --env CARGO_TARGET_DIR=/work/target \
  --env CARGO_HOME=/work/.cargo \
  rust:1.47-slim-buster \
  bash -c 'cd /work; cargo build --release --bin proxy-cli'

trap "rm -rf /root/workspace/docker-scratch || true" EXIT

rm -rf /root/workspace/docker-scratch || true
mkdir -p /root/workspace/docker-scratch
cp /root/workspace/target/release/kitsune-p2p-proxy /root/workspace/docker-scratch/
cp /root/workspace/target/release/proxy-cli /root/workspace/docker-scratch/

cat << EOF > /root/workspace/docker-scratch/Dockerfile
FROM debian:buster-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends iputils-ping iproute2 net-tools procps

COPY ./* /usr/bin/
EOF

docker build -t hc-containernet-base /root/workspace/docker-scratch
/holochain/containernet/containernet-script.py
