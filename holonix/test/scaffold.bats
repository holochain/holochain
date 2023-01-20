#!/usr/bin/env bats

@test "holochain-create is in PATH and can be called" {
 result="$( holochain-create --version )"
 echo $result
 [[ "$result" == * ]]
}
