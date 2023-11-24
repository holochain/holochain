#!/usr/bin/env sh

HOLOCHAIN_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
exec nix develop "$HOLOCHAIN_DIR"#${flavor:-coreDev} \
    --override-input versions "$HOLOCHAIN_DIR"/versions/${versions:-weekly} ${@}
