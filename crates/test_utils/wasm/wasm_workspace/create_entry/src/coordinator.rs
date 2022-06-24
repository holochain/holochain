use crate::integrity::*;
use hdk::prelude::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityCreateEntry(crate::integrity::EntryTypes),
}

fn post() -> Post {
    Post("foo".into())
}

fn new_post() -> EntryZomes {
    EntryZomes::IntegrityCreateEntry(EntryTypes::Post(Post("foo".into())))
}

fn msg() -> Msg {
    Msg("hello".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<ActionHash> {
    let post = new_post();
    HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            EntryInput::App(AppEntry {
                entry_def_index: ScopedEntryDefIndex::try_from(&post)?,
                visibility: EntryVisibility::from(&post),
                entry: post.try_into().unwrap(),
            }),
            // This is used to test many conductors thrashing creates between
            // each other so we want to avoid retries that make the test take
            // a long time.
            ChainTopOrdering::Relaxed,
        )?)
    })
}

#[hdk_extern]
fn create_post(post: Post) -> ExternResult<ActionHash> {
    hdk::prelude::create_entry(&EntryZomes::IntegrityCreateEntry(
        crate::integrity::EntryTypes::Post(post),
    ))
}

#[hdk_extern]
fn delete_post(post_hash: ActionHash) -> ExternResult<ActionHash> {
    hdk::prelude::delete_entry(post_hash)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Record>> {
    get(hash_entry(&post())?, GetOptions::content())
}

#[hdk_extern]
fn get_entry_twice(_: ()) -> ExternResult<Vec<Option<Record>>> {
    HDK.with(|h| {
        h.borrow().get(vec![
            GetInput::new(
                hash_entry(&post())?.into(),
                GetOptions::content()
            );
            2
        ])
    })
}

#[hdk_extern]
fn get_post(hash: ActionHash) -> ExternResult<Option<Record>> {
    get(hash, GetOptions::content())
}

#[hdk_extern]
fn create_msg(_: ()) -> ExternResult<ActionHash> {
    use EntryTypes::*;
    use EntryZomes::*;
    hdk::prelude::create_entry(IntegrityCreateEntry(Msg(msg())))
}

#[hdk_extern]
fn create_priv_msg(_: ()) -> ExternResult<ActionHash> {
    use EntryTypes::*;
    use EntryZomes::*;
    hdk::prelude::create_entry(&IntegrityCreateEntry(PrivMsg(crate::integrity::PrivMsg(
        "Don't tell anyone".into(),
    ))))
}

#[hdk_extern]
fn get_activity(
    input: holochain_test_wasm_common::AgentActivitySearch,
) -> ExternResult<AgentActivity> {
    get_agent_activity(input.agent, input.query, input.request)
}

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to accept_cap_claim so other agents can send us claims
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.name, "create_entry".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}

/// Create a post entry then
/// create another post through a
/// call
#[hdk_extern]
fn call_create_entry(_: ()) -> ExternResult<ActionHash> {
    // Create an entry directly via. the hdk.
    hdk::prelude::create_entry(&new_post())?;
    // Create an entry via a `call`.
    let zome_call_response: ZomeCallResponse = call(
        CallTargetCell::Local,
        zome_info()?.name,
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e.into()))?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(wasm_error!(WasmErrorInner::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            ))))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(_) => call_create_entry(()),
        ZomeCallResponse::CountersigningSession(e) => Err(wasm_error!(WasmErrorInner::Guest(
            format!("Countersigning session failed: {}", e)
        ))),
    }
}

#[hdk_extern]
fn call_create_entry_remotely(agent: AgentPubKey) -> ExternResult<ActionHash> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent.clone(),
        zome_info()?.name,
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e.into()))?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(wasm_error!(WasmErrorInner::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            ))))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(_) => call_create_entry_remotely(agent),
        ZomeCallResponse::CountersigningSession(e) => Err(wasm_error!(WasmErrorInner::Guest(
            format!("Countersigning session failed: {}", e)
        ))),
    }
}

#[hdk_extern]
fn must_get_valid_record(action_hash: ActionHash) -> ExternResult<Record> {
    hdk::prelude::must_get_valid_record(action_hash)
}

/// Same as above but doesn't recurse on network errors.
#[hdk_extern]
fn call_create_entry_remotely_no_rec(agent: AgentPubKey) -> ExternResult<ActionHash> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent.clone(),
        zome_info()?.name,
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e.into()))?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(wasm_error!(WasmErrorInner::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            ))))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(e) => Err(wasm_error!(WasmErrorInner::Guest(format!(
            "Network Error: {}",
            e
        )))),
        ZomeCallResponse::CountersigningSession(e) => Err(wasm_error!(WasmErrorInner::Guest(
            format!("Countersigning session failed: {}", e)
        ))),
    }
}
