use hdk::prelude::*;

#[hdk_extern(infallible)]
fn zome_fn() -> ExternResult<String> {
    Ok("should not be wrapped".into())
}

fn main() {}
