use hdk::prelude::*;

#[hdk_entry(id = "post", required_validations = 5)]
struct Post(String);

#[hdk_entry(id = "msg", required_validations = 5)]
struct Msg(String);

entry_defs![Post::entry_def(), Msg::entry_def()];

fn post() -> Post {
    Post("foo".into())
}

fn msg() -> Msg {
    Msg("hi".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&post())
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Element>> {
    get(
        hash_entry(&post())?,
        GetOptions::latest(),
    )
}

#[hdk_extern]
fn update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = hdk::prelude::create_entry(&post())?;
    hdk::prelude::update_entry(header_hash, &post())
}

#[hdk_extern]
/// Updates to a different entry, this will fail
fn invalid_update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = hdk::prelude::create_entry(&post())?;
    hdk::prelude::update_entry(header_hash, &msg())
}
