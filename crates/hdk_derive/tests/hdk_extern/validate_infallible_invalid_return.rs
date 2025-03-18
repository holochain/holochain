use hdk::prelude::*;

#[hdk_extern(infallible)] 
fn validate() -> String {
    "wrong return type".into()
}

fn main() {}
