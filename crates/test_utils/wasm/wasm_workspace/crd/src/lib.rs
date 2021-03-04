use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn create(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing)
}

/// `read` seems to be a reserved worked that causes SIGSEGV invalid memory reference when used as `#[hdk_extern]`
#[hdk_extern]
fn reed(header_hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(header_hash, GetOptions::latest())
}

#[hdk_extern]
fn delete(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete_entry(header_hash)
}
