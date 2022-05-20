use hdk::prelude::*;
use integrity_zome::Post;
use test_wasm_integrity_zome as integrity_zome;

enum EntryZomes {
    IntegrityZome(integrity_zome::EntryTypes),
}

impl TryFrom<&EntryZomes> for EntryDefIndex {
    type Error = WasmError;
    fn try_from(e: &EntryZomes) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<&&EntryZomes> for EntryDefIndex {
    type Error = WasmError;
    fn try_from(e: &&EntryZomes) -> Result<Self, Self::Error> {
        Self::try_from(*e)
    }
}

impl TryFrom<&EntryZomes> for Entry {
    type Error = WasmError;

    fn try_from(value: &EntryZomes) -> Result<Self, Self::Error> {
        match value {
            EntryZomes::IntegrityZome(e) => Entry::try_from(e),
        }
    }
}
impl TryFrom<EntryZomes> for Entry {
    type Error = WasmError;

    fn try_from(value: EntryZomes) -> Result<Self, Self::Error> {
        Entry::try_from(&value)
    }
}

#[hdk_extern]
fn get_entry(hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(hash, GetOptions::content())
}

#[hdk_extern]
fn create_post(post: Post) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&EntryZomes::IntegrityZome(
        integrity_zome::EntryTypes::Post(post),
    ))
}
