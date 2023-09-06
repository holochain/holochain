
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

@test "expected hc-sandbox to clean, create, and run a sandbox" {
  set -muEeo pipefail

  echo pass | hc-sandbox --piped create -n1
  (echo pass | hc-sandbox --piped run 0) &

  declare result
  result=0

  # succedd if we can reach the admin interface within 10 seconds
  for i in `seq 1 10`; do
    sleep 1
    (
      set -eE
      if [[ -f .hc_live_0 ]]; then
        hc-sandbox --piped call --running $(cat .hc_live_0) list-apps
      else
        exit 1
      fi
    ) && {
      result=0
      continue
    } || {
      result=$?
    }
  done

  echo rc=$result

  # ideally we'd know all children but because of double-forks we don't
  pkill -9 hc-sandbox
  pkill -9 holochain
  pkill -9 lair-keystore
}