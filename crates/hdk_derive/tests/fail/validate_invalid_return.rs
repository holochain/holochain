use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern] 
fn validate() -> String { //~ ERROR `validate` must return `ExternResult<ValidateCallbackResult>`
    "wrong return type".into()
}

fn main() {}
