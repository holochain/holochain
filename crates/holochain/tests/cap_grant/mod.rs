use hdk::prelude::{wasm_error, ActionType, EntryType, Record};
use hdk::prelude::{AppEntryDef, WasmError, ZomeIndex};
use hdk::prelude::{
    CapAccess, CapGrantEntry, Entry, EntryDef, EntryDefIndex, EntryVisibility, ExternIO,
    GrantedFunction, GrantedFunctions,
};
use holo_hash::{ActionHash, AgentPubKey};
use holochain::sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile, SweetInlineZomes};
use holochain::test_utils::inline_zomes::AppString;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::{
    Call, CallTarget, ChainQueryFilter, ChainTopOrdering, CreateInput, EntryDefLocation,
    FunctionName, InlineZomeResult, ZomeCallResponse,
};
use std::collections::BTreeSet;
use std::time::Duration;
use holochain::test_utils::consistency_60s;

#[tokio::test(flavor = "multi_thread")]
async fn grant_access_to_multiple_zome_functions() {
    let string_entry_def = EntryDef::from_id("string");

    let inline_zomes = SweetInlineZomes::new(vec![string_entry_def], 0)
        .function("foo", move |api, _: ()| {
            println!("debug zome {:?}", api.zome_info(())?);
            println!("agent info {:?}", api.agent_info(())?);

            println!("hello from foo");

            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::app(
                    AppString(api.agent_info(())?.agent_initial_pubkey.to_string())
                        .try_into()
                        .unwrap(),
                )
                .unwrap(),
                ChainTopOrdering::default(),
            ))?;

            Ok(hash)
        })
        .function("bar", move |_, _: ()| Ok(()))
        .function("list_entries", move |api, _: ()| {
            println!("list_entries: debug zome {:?}", api.zome_info(())?);
            println!("list_entries: zome id {:?}", api.zome_info(())?.id);

            let result: Vec<Record> = api.query(ChainQueryFilter::new().include_entries(true).action_type(ActionType::Create)).unwrap();

            println!("Found {:?}", result);

            api.query(
                ChainQueryFilter::new().entry_type(EntryType::App(AppEntryDef::new(
                    EntryDefIndex(0),
                    api.zome_info(())?.id,
                    EntryVisibility::Public,
                ))),
            )
            .map_err(Into::into)

            // api.query(ChainQueryFilter::new().action_type(ActionType::Create))
            //     .map_err(Into::into)
        })
        .function("grant_two_capabilities", move |api, _: ()| {
            println!("debug zome {:?}", api.zome_info(())?);

            let mut functions = BTreeSet::<GrantedFunction>::new();
            functions.insert((api.zome_info(())?.name, FunctionName("foo".to_owned())));
            functions.insert((api.zome_info(())?.name, FunctionName("bar".to_owned())));

            let granted_functions: GrantedFunctions = GrantedFunctions::Listed(functions);

            api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(CapGrantEntry::new(
                    "".to_owned(),
                    CapAccess::Unrestricted,
                    granted_functions,
                )),
                ChainTopOrdering::default(),
            ))?;

            Ok(())
        })
        .function(
            "call_foo",
            move |api, agent: AgentPubKey| -> InlineZomeResult<ActionHash> {
                println!("debug zome {:?}", api.zome_info(())?);
                println!("call_foo: agent info {:?}", api.agent_info(())?);

                let responses = api.call(vec![Call::new(
                    CallTarget::NetworkAgent(agent),
                    api.zome_info(())?.name,
                    FunctionName("foo".to_owned()),
                    None,
                    ExternIO::encode(()).unwrap(),
                )])?;

                match responses.first().unwrap() {
                    ZomeCallResponse::Ok(v) => Ok(v
                        .decode::<ActionHash>()
                        .map_err(|e| wasm_error!(e))
                        .unwrap()),
                    _ => {
                        panic!("Unexpected response from call remote {:?}", responses);
                    }
                }
            },
        );

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(inline_zomes.0).await;
    let app = conductors.setup_app("cap", &[dna]).await.unwrap();

    let ((cell,), (cell2,)) = app.into_tuples();
    let zome_a = cell.zome(SweetInlineZomes::COORDINATOR);
    let zome_b = cell2.zome(SweetInlineZomes::COORDINATOR);

    conductors.exchange_peer_info().await;

    let _: () = conductors[0]
        .call(&zome_a, "grant_two_capabilities", ())
        .await;

    let entries: Vec<Record> = conductors[0].call(&zome_a, "list_entries", ()).await;
    println!("got records {:?}", entries);
    // Should be empty, nothing created yet
    assert!(entries.is_empty());

    let _: ActionHash = conductors[1]
        .call(&zome_b, "call_foo", cell.agent_pubkey())
        .await;

    tokio::time::sleep(Duration::from_secs(15)).await;

    let entries: Vec<Record> = conductors[0].call(&zome_a, "list_entries", ()).await;
    println!("a: got records {:?}", entries);

    consistency_60s(&[cell, cell2]).await;

    tokio::time::sleep(Duration::from_secs(15)).await;

    let entries_b: Vec<Record> = conductors[1].call(&zome_b, "list_entries", ()).await;
    println!("b: got records {:?}", entries_b);
}
