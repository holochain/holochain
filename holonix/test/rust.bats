#!/usr/bin/env bats

# the mozilla nightly and upstream nightly can get out of sync!
# rustc reports the version that needs to match https://rust-lang.github.io/rustup-components-history/
@test "rustc version" {
 result="$( rustc --version )"
 echo $result
 [[ "$result" == * ]]
}

# the rust fmt version should be roughly the rustc version
# most importantly fmt should exist
@test "fmt version" {
 result="$( cargo fmt --version )"
 echo $result
 [[ "$result" == *-stable* ]]
}

# RUSTFLAGS should be set correctly
@test "RUSTFLAGS value" {
 result="$( echo $RUSTFLAGS )"
 echo $result
 [[ "$result" == '-D warnings -Z external-macro-backtrace -Z thinlto -C codegen-units=10' || "$result" == '-D warnings -C codegen-units=10' ]]
}
