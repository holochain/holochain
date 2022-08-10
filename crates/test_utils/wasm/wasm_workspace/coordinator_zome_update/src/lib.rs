use hdk::prelude::*;
use integrity_zome::Post;
use test_wasm_integrity_zome as integrity_zome;
use test_wasm_music_integrity_zome as music_zome;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityZome(integrity_zome::EntryTypes),
    MusicTypes(music_zome::MusicTypes),
}

#[hdk_dependent_link_types]
enum LinkTypes {
    IntegrityZome(integrity_zome::LinkTypes),
    MusicZome(music_zome::LinkTypes),
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
