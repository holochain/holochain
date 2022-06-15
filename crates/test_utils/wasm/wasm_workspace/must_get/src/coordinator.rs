use hdk::prelude::*;

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn must_get_header(header_hash: HeaderHash) -> ExternResult<SignedHeaderHashed> {
    hdk::prelude::must_get_header(header_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}
