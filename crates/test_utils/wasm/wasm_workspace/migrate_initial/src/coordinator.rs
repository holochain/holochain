use crate::integrity::*;
use hdk::prelude::*;

/// Returned by [`prepare_migration_summary`] so the caller can assemble the `init_properties`
/// payload for the migration target DNA.
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct MigrationSummary {
    /// Serialised values from all [`MyType`] entries on the chain.
    pub summary: Vec<String>,
    /// Signature over `summary` using the agent's own key.
    pub signature: Signature,
}

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

/// Collect all [`MyType`] values and sign them with the agent's own key.
///
/// Call this before calling [`close_chain_for_new`] to obtain the summary and signature.
/// Combine the returned data with the `close_hash` returned by [`close_chain_for_new`] and the
/// `prev_dna_hash` to build the `InitPropertiesPayload` that the migration target DNA expects as
/// `init_properties`.
#[hdk_extern]
fn prepare_migration_summary(_: ()) -> ExternResult<MigrationSummary> {
    let agent_info = agent_info()?;
    let records = query(
        ChainQueryFilter::new()
            .entry_type(EntryTypesUnit::MyType.try_into().unwrap())
            .include_entries(true),
    )?;
    let values: Vec<String> = records
        .into_iter()
        .filter_map(|r| {
            let entry = r.entry.into_option()?;
            MyType::try_from(entry).ok().map(|t| t.value)
        })
        .collect();

    let signature = sign(agent_info.agent_initial_pubkey, values.clone())?;

    Ok(MigrationSummary {
        summary: values,
        signature,
    })
}

#[hdk_extern]
fn close_chain_for_new(dna_hash: DnaHash) -> ExternResult<ActionHash> {
    close_chain(Some(dna_hash.into()))
}
