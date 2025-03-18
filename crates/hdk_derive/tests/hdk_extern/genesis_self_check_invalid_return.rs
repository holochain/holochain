use hdk::prelude::*;

#[hdk_extern]
fn genesis_self_check() -> String {
    "wrong return type".into()
}

fn main() {}
