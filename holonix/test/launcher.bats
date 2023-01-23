#!/usr/bin/env bats

setup() {
  BATS_TMPDIR="$(mktemp -d)"
  cd "${BATS_TMPDIR:?}"
}

teardown() {
  cd ..
  rm -rf "${BATS_TMPDIR:?}"
}

@test "expected hc-launch to be available" {
  result="$(hc-launch --version)"
  echo $result
  [[ "$result" == "holochain_cli_launch"* ]]
}

# @test "hApp launch hc-launch" {
#   # TODO
#   :
# }
