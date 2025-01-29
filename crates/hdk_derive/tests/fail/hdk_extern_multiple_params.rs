use hdk::{map_extern, map_extern_infallible};
use hdk_derive::hdk_extern;

#[hdk_extern] //~ ERROR hdk_extern functions must take a single parameter or none
fn multiple_params(a: String, b: i32) -> ExternResult<String> {
    Ok(format!("{}{}", a, b))
}

fn main() {}
