use hdk::prelude::*;

#[hdk_extern] 
fn zome_fn() -> String {
    "should be wrapped in ExternResult".into()
}

fn main() {}
