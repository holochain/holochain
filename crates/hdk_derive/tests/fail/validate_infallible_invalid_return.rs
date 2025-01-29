use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern(infallible)] 
fn validate() -> String { //~ ERROR `validate` must return `ValidateCallbackResult`
    "wrong return type".into()
}

fn main() {}
