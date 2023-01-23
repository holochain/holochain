#!/usr/bin/env bats

# the perf version should be some number most importantly perf should exist
@test "perf version" {
 result="$( perf --version )"
 echo $result
 [[ "$result" == "perf version "* ]]
}
