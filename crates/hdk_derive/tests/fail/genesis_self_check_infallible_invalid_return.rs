use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern(infallible)] 
fn genesis_self_check() -> String { //~ ERROR `genesis_self_check` must return `ValidateCallbackResult`
    "wrong return type".into()
}

fn main() {}
