#!/usr/bin/env bats

@test "shellHook is evaluated" {
    [ -n "$HOLOCHAIN_DEVSHELL" ]
}