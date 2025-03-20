use hdk_derive::*;

#[hdk_extern(infallible)] 
fn genesis_self_check() -> ExternResult<String> {
    Ok("wrong return type".into())
}

fn main() {}
