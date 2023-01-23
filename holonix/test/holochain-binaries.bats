#!/usr/bin/env bats

@test "expected holochain available" {
  result="$(holochain --version)"
  echo $result
  [[ "$result" == "holochain"* ]]
}

@test "expected hc version available" {
  result="$(hc --version)"
  echo $result
  [[ "$result" == "holochain_cli"* ]]
}

@test "expected lair-keystore available" {
  result="$(lair-keystore --version)"
  echo $result
  [[ "$result" =~ ^lair[-_]keystore.* ]]
}

@test "expected kitsune-p2p-proxy available" {
  result="$(kitsune-p2p-proxy --version || kitsune-p2p-tx2-proxy --version)"
  echo $result
  [[ "$result" == "kitsune-p2p"* ]]
}
