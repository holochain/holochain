use hdk_derive::*;

#[hdk_extern] 
fn validate() -> String {
    "wrong return type".into()
}

fn main() {}
