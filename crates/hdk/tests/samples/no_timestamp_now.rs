use hdk::prelude::*;

// Intended to be a regression test for https://github.com/holochain/holochain/issues/3003
fn main() {
    // Should not be available, will crash in a wasm call
    Timestamp::now();
}
