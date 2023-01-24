#!/usr/bin/env bats

@test "temp dir" {
 [[ $TMP == /tmp/tmp.* ]]
 [[ $TMPDIR == /tmp/tmp.* ]]
 [[ $TMP == $TMPDIR ]]
}

@test "rust backtrace is set in shell" {
  [ "$RUST_BACKTRACE" == "1" ]
}

@test "CARGO_HOME is set and not directly at root" {
  [ "$CARGO_HOME" != "" ] && [ "$CARGO_HOME" != "/.cargo" ]
}

@test "hn-introspect lists holochain" {
 hn-introspect | egrep '.*- holochain-.+: (https|git)://.*holochain.*'
}

@test "exclude components" {
 nix-shell --pure ./default.nix --arg include '{ holochainBinaries = false; node = false; happs = false; }' --run '
    for cmd in holochain hc node; do
        if type -f $cmd; then
            echo error: did not expect to find $cmd
            exit 1
        fi
    done
 '
}

@test "error on obsolete holochainVersion attribute" {
    run nix-shell --argstr holochainVersionId 'custom' --arg holochainVersion '{ cargoSha256 = "..."; }'
    [ $status -ne 0 ]
    [[ "$output" =~ "error: The following attributes found in the 'holochainVersion' set are no longer supported:"* ]]
}
