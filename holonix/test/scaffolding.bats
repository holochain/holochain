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

# @test "hApp scaffolding with hn-init" {
#   hn-init
#   cd my-app
#   nix-shell --pure --run '
#       npm i
#       npm run test
#       npm run package
#   '
# }
