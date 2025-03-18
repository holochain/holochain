use hdk::prelude::*;

#[hdk_extern(infallible)] 
fn init() -> String {
    "wrong return type".into()
}

fn main() {}
