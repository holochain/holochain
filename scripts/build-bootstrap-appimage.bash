#!/bin/bash

# Checks out a specific bootstrap (by holochain version) and builds it
# into a linux portable .AppImage file.
# Usage: ./build-bootstrap-appimage.bash [version]
#  E.g.: ./build-bootstrap-appimage.bash 0.1.5

# report errors
set -eEo pipefail

# make sure we were passed a version
if [[ "${1}x" == "x" ]]; then
  echo "expected version, e.g.: ./build-bootstrap-appimage.bash 0.1.5"
  exit 127
fi

# get a temp directory
dir=$(mktemp -d 2>/dev/null || mktemp -d -t 'build-bs-appimage')

# make sure the temp dir is cleaned up
trap "rm -rf '${dir}'" 0

# remember our current dir
pushd .

# move into the temp dir
cd "${dir}"

# write out our appimage recipe
cat << EOF > AppImageBuilder.yml
version: 1
AppDir:
  path: ./AppDir
  app_info:
    id: org.holochain.bootstrap
    name: bootstrap
    icon: holochain
    version: ${1}
    exec: usr/bin/kitsune-bootstrap
    exec_args: \$@
  files:
    include:
    - /usr/bin/kitsune-bootstrap
    - /usr/lib/x86_64-linux-gnu/libssl.so.1.1
    - /usr/lib/x86_64-linux-gnu/libcrypto.so.1.1
    - /lib/x86_64-linux-gnu/libgcc_s.so.1
    - /lib/x86_64-linux-gnu/libm.so.6
    exclude:
    - usr/share/man
    - usr/share/doc/*/README.*
    - usr/share/doc/*/changelog.*
    - usr/share/doc/*/NEWS.*
    - usr/share/doc/*/TODO.*
AppImage:
  arch: x86_64
  update-information: guess
EOF

# write out our Dockerfile
cat << EOF > Dockerfile
FROM appimagecrafters/appimage-builder:latest AS BASE

SHELL ["/bin/bash", "-c"]

ADD AppImageBuilder.yml .

RUN apt-get update && \\
  apt-get install -y curl git libssl-dev && \\
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \\
  git clone https://github.com/holochain/holochain.git && \\
  cd holochain && \\
  git checkout holochain-${1} && \\
  cd crates/kitsune_p2p/bootstrap && \\
  source ~/.cargo/env && \\
  cargo build --release && \\
  cd ../../../.. && \\
  mkdir -p AppDir/usr/bin && \\
  cp holochain/target/release/kitsune-bootstrap AppDir/usr/bin/kitsune-bootstrap && \\
  mkdir -p /usr/share/icons/hicolor/284x284/apps && \\
  curl \\
    -o /usr/share/icons/hicolor/284x284/apps/holochain.png \\
    https://raw.githubusercontent.com/holochain/launcher/e165f7848711e314fff66bbc8dffcbe08e93b0a1/public/img/Square284x284Logo.png && \\
  appimage-builder --recipe AppImageBuilder.yml --skip-script --skip-tests

FROM scratch

COPY --from=BASE \
  ./bootstrap-${1}-x86_64.AppImage ./bootstrap-${1}-x86_64.AppImage
EOF

# build the app image
docker build -o result .

# switch back to our original directory
popd

# move the built image to the current dir
mv "${dir}/result/bootstrap-${1}-x86_64.AppImage" .

# done
echo "Done."
