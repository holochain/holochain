#!/bin/bash

# this script is executed within the containernet VM to execute the test script

# sane bash errors
set -eEuo pipefail

# set up a working directory for cargo build
mkdir -p /root/workspace
cd /root/workspace

# we can't use the host system's target directory safely
# so sync just the source files to our workspace
rsync -av \
  --exclude 'target/' \
  --exclude '.cargo/' \
  --exclude '.git/' \
  --exclude '.wasm_cache/' \
  --exclude '.wasm_target/' \
  --exclude '.vagrant/' \
  --exclude 'containernet/' \
  /holochain/ /root/workspace/

# build the kitsune-p2p-proxy binary
docker run -v /root/workspace:/work \
  --env CARGO_TARGET_DIR=/work/target \
  --env CARGO_HOME=/work/.cargo \
  rust:1.47-slim-buster \
  bash -c 'cd /work; cargo build --release --bin kitsune-p2p-proxy'

# build the proxy-cli binary
docker run -v /root/workspace:/work \
  --env CARGO_TARGET_DIR=/work/target \
  --env CARGO_HOME=/work/.cargo \
  rust:1.47-slim-buster \
  bash -c 'cd /work; cargo build --release --bin proxy-cli'

# set up a bash trap to clean up our docker build scratch space
trap "rm -rf /root/workspace/docker-scratch || true" EXIT

# incase the trap didn't work - also clean up the scratch space
rm -rf /root/workspace/docker-scratch || true

# make sure the scratch space exists
mkdir -p /root/workspace/docker-scratch

# copy in our binaries
cp /root/workspace/target/release/kitsune-p2p-proxy /root/workspace/docker-scratch/
cp /root/workspace/target/release/proxy-cli /root/workspace/docker-scratch/

# write the Dockerfile
# use buster-slim because we already have the layer downloaded
# (it is the base of rust:1.47-slim-buster)
cat << EOF > /root/workspace/docker-scratch/Dockerfile
FROM debian:buster-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends iputils-ping iproute2 net-tools procps

COPY ./* /usr/bin/
EOF

# build the docker container
docker build -t hc-containernet-base /root/workspace/docker-scratch

# cleanup any previous containernet execution
mn -c

# execute our containernet test script
/holochain/containernet/containernet-script.py
