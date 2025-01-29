use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern(infallible)] 
fn infallible() -> ExternResult<String> { //~ ERROR infallible functions should return the inner type directly
    Ok("should not be wrapped".into())
}

fn main() {}
