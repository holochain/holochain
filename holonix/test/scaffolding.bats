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

  print_version() {
    hc-scaffold --version
  }

  setup_and_build_hello_world() {
    print_version

    hc-scaffold example hello-world
    cd hello-world

    nix develop --command bash -c "
      set -e
      npm install
      npm test
      "
  }

  setup_and_build_hello_world
}
