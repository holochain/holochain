#!/usr/bin/env bats

@test "flamegraph test" {
 files=$(ls /nix/store/*-FlameGraph*/bin/flamegraph.pl 2> /dev/null | wc -l)

 [ "$files" -ne "0" ]

}
