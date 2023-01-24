#!/usr/bin/env bats

@test "github-release version" {
 result="$( github-release version )"
 [ "$result" == "1.2.4" ]
}
