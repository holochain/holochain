
#!/usr/bin/env bats

setup() {
  BATS_TMPDIR="$(mktemp -d)"
  cd "${BATS_TMPDIR:?}"
}

teardown() {
  cd ..
  rm -rf "${BATS_TMPDIR:?}"
  hc-sandbox clean
}

@test "expected hc-sandbox to be available" {
  result="$(type -f hc-sandbox)"
  echo $result
  [[ "$result" == *"hc-sandbox" ]]
}

@test "expected hc-sandbox to clean, create, and run a sandbox" {
  set -x
  hc-sandbox clean
  echo pass | hc-sandbox --piped create -n1
  echo pass | hc-sandbox --piped run 0
}