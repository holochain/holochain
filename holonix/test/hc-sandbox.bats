
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
  set -uEeo pipefail

  echo pass | hc-sandbox --piped create -n1
  (echo pass | hc-sandbox --piped run 0) &

  declare result
  result=0

  # succedd if we can reach the admin interface within 10 seconds
  for i in `seq 1 10`; do
    sleep 1
    (
      if [[ -f .hc_live_0 ]]; then
        hc-sandbox --piped call --running $(cat .hc_live_0) list-apps
      else
        exit 1
      fi
    ) && {
      result=0
      break
    } || {
      result=$?
    }
  done

  echo rc=$result

  # ideally we'd know all children but because of double-forks we don't
  # these might fail and then the test will hang as the nix-daemon waits for all processes to end
  set +e
  killall -v -u "$(whoami)" -9 hc-sandbox
  killall -v -u "$(whoami)" -9 holochain
  killall -v -u "$(whoami)" -9 lair-keystore
}