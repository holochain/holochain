#! /usr/bin/env bats

@test "all-pinned comes back empty" {

 cd ./rust/manifest/list-unpinned/examples/all-pinned
 run hn-rust-manifest-list-unpinned

 [ -z "$output" ]
}

@test "one-unpinned comes back with one result" {

 cd ./rust/manifest/list-unpinned/examples/one-unpinned
 run hn-rust-manifest-list-unpinned

 [ "$output" == 'structopt = "0.2.15"' ]
}
