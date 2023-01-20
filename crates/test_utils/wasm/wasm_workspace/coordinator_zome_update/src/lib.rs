use hdk::prelude::*;
use integrity_zome::Post;
use test_wasm_integrity_zome as integrity_zome;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityZome(integrity_zome::EntryTypes),
}

#[hdk_extern]
fn get_entry(hash: ActionHash) -> ExternResult<Option<Record>> {
    get(hash, GetOptions::content())
}

#[hdk_extern]
fn create_post(post: Post) -> ExternResult<ActionHash> {
    hdk::prelude::create_entry(&EntryZomes::IntegrityZome(
        integrity_zome::EntryTypes::Post(post),
    ))
}
