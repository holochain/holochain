use hdk::prelude::*;

#[hdk_extern]
fn get_entry(hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(hash, GetOptions::content())
}
