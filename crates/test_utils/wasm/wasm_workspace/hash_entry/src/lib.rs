use hdk3::prelude::*;

#[hdk_extern]
fn hash_entry(input: HashEntryInput) -> ExternResult<EntryHash> {
    Ok(hash_entry!(input)?)
}
