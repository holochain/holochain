use hdk::prelude::*;

fn main() {
    // Should not be available, will crash in a wasm call
    Timestamp::now();
}
