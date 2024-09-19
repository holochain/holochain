use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
fn create() -> ExternResult<ActionHash> {
    create_entry(EntryTypes::MyType(MyType {
        value: "test".to_string(),
    }))
}

#[hdk_extern]
fn get_all_my_types() -> ExternResult<Vec<MyType>> {
    let records = query(
        ChainQueryFilter::new()
            .entry_type(EntryTypesUnit::MyType.try_into().unwrap())
            .include_entries(true),
    )?;

    let my_types = records
        .into_iter()
        .filter_map(|r| {
            let entry = r.entry.into_option()?;
            MyType::try_from(entry).ok()
        })
        .collect();

    Ok(my_types)
}

#[hdk_extern]
fn close_chain_for_new(dna_hash: DnaHash) -> ExternResult<ActionHash> {
    close_chain(Some(dna_hash.into()))
}
