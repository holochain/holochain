use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern(infallible)] 
fn init() -> String { //~ ERROR `init` must return `InitCallbackResult`
    "wrong return type".into()
}

fn main() {}
