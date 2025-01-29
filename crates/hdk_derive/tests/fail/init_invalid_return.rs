use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern]
fn init() -> String { //~ ERROR `init` must return `ExternResult<InitCallbackResult>`
    "hello".into()
}

fn main() {}
