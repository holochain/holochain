#!/usr/bin/env bats

setup() {
  BATS_TMPDIR="$(mktemp -d)"
  cd "${BATS_TMPDIR:?}"
}

teardown() {
  cd ..
  rm -rf "${BATS_TMPDIR:?}"
}

@test "expected hc-scaffold to be available" {
  result="$(hc-scaffold --version)"
  echo $result
  [[ "$result" == "holochain_scaffolding_cli"* ]]
}

@test "expected scaffold an example to succeed" {
  set -e
  TEMP_PATH="$GITHUB_WORKSPACE/temporary"

  cleanup_tmp() {
    rm -rf "${TEMP_PATH:?}/$1"
  }

  print_version() {
    echo "$(hc-scaffold --version)"
  }

  setup_and_build_hello_world() {
    print_version
    mkdir TEMP_PATH
    cd $TEMP_PATH

    hc-scaffold example hello-world
    cd hello-world

    # TODO: override holochain version dynamically
    nix develop --command bash -c "
      set -e
      npm install
      npm test
      "
    cd ..
    cleanup_tmp hello-world
  }

  setup_and_build_hello_world
}
