use crate::integrity::*;
use hdk::prelude::*;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Properties {
    pub prev_dna_hash: holo_hash::DnaHash,
}

#[hdk_extern]
fn init() -> ExternResult<InitCallbackResult> {
    let properties: Properties = dna_info()
        .unwrap()
        .modifiers
        .properties
        .try_into()
        .map_err(|_| {
            wasm_error!(WasmErrorInner::Guest(
                "Could not deserialize properties".to_string()
            ))
        })?;

    // TODO: must get close_hash from init context, which is currently not possible.
    let close_hash = ActionHash::from_raw_36(vec![0; 36]);
    open_chain(properties.prev_dna_hash.clone().into(), close_hash)?;

    let my_agent_info = agent_info()?;
    let response = call(
        CallTargetCell::OtherCell(CellId::new(
            properties.prev_dna_hash,
            my_agent_info.agent_initial_pubkey,
        )),
        "migrate_initial",
        "get_all_my_types".into(),
        None,
        (),
    )?;
    match response {
        ZomeCallResponse::Ok(my_types) => {
            let my_types: Vec<MyOldType> = my_types.decode().map_err(|e| {
                wasm_error!(WasmErrorInner::Guest(format!(
                    "Unexpected type in response from get_all_my_types: {:?}",
                    e
                )))
            })?;
            for my_type in my_types {
                create_entry(EntryTypes::MyType(MyType {
                    value: my_type.value,
                    amount: 0,
                }))?;
            }
        }
        _ => {
            return Err(wasm_error!(WasmErrorInner::Guest(
                "Failed to get all 'MyType's".to_string()
            )));
        }
    }

    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn create(_: ()) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::MyType(MyType {
        value: "test new".to_string(),
        amount: 4,
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
