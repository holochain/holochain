use hdk3::prelude::*;

#[hdk_extern]
fn entry_hash(input: EntryHashInput) -> ExternResult<EntryHash> {
    Ok(entry_hash!(input)?)
}
