use hdk3::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn create(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing)
}

#[hdk_extern]
fn read(header_hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(header_hash, GetOptions::latest())
}

#[hdk_extern]
fn delete(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete_entry(header_hash)
}
