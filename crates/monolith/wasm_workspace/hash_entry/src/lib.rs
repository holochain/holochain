use crate::hdk3::prelude::*;

#[hdk_extern]
fn hash_entry(input: HashEntryInput) -> ExternResult<EntryHash> {
    Ok(hdk3::prelude::hash_entry(&input.into_inner())?)
}
