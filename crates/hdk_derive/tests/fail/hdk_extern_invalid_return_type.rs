use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern] 
fn regular_function() -> String { //~ ERROR functions marked with #[hdk_extern] must return `ExternResult<String>`
    "should be wrapped in ExternResult".into()
}

fn main() {}
