use hdk::prelude::*;

#[hdk_extern(infallible)]
fn post_commit(_: Vec<HeaderHash>) { }
