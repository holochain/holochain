
#!/usr/bin/env bats

setup() {
  BATS_TMPDIR="$(mktemp -d)"
  cd "${BATS_TMPDIR:?}"
}

teardown() {
  cd ..
  rm -rf "${BATS_TMPDIR:?}"
}

@test "expected hc-sandbox to be available" {
  result="$(type -f hc-sandbox)"
  echo $result
  [[ "$result" == *"hc-sandbox" ]]
}
