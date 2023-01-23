#!/usr/bin/env bats

# the clippy version should be roughly the rustc version
# most importantly clippy should exist
@test "clippy version" {
 result="$( cargo clippy --version )"
 echo $result
 [[ "$result" == *0.1.* ]]
}
