use hdk::prelude::*;
use integrity_zome::Msg;
use integrity_zome::Post;
use integrity_zome::PrivMsg;
use test_wasm_integrity_zome as integrity_zome;

fn post() -> Post {
    Post("foo".into())
}

fn msg() -> Msg {
    Msg("hello".into())
}

fn priv_msg() -> PrivMsg {
    PrivMsg("Don't tell anyone".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    let post = post();
    let input = CreateInput::new(
        (&post).into(),
        post.try_into().unwrap(),
        // This is used to test many conductors thrashing creates between
        // each other so we want to avoid retries that make the test take
        // a long time.
        ChainTopOrdering::Relaxed,
    );
    // input.zome_name = Some("integrity_zome".into());
    HDK.with(|h| h.borrow().create(input))
}

#[hdk_extern]
fn create_post(post: crate::Post) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&post)
}

#[hdk_extern]
fn delete_post(post_hash: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_entry(post_hash)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Element>> {
    get(hash_entry(&post())?, GetOptions::content())
}

#[hdk_extern]
fn get_entry_twice(_: ()) -> ExternResult<Vec<Option<Element>>> {
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
fn get_post(hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(hash, GetOptions::content())
}

#[hdk_extern]
fn create_msg(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&msg())
}

#[hdk_extern]
fn create_priv_msg(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&priv_msg())
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
fn call_create_entry(_: ()) -> ExternResult<HeaderHash> {
    // Create an entry directly via. the hdk.
    hdk::prelude::create_entry(&post())?;
    // Create an entry via a `call`.
    let zome_call_response: ZomeCallResponse = call(
        CallTargetCell::Local,
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode()?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(WasmError::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            )))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(_) => call_create_entry(()),
        ZomeCallResponse::CountersigningSession(e) => Err(WasmError::Guest(format!(
            "Countersigning session failed: {}",
            e
        ))),
    }
}

#[hdk_extern]
fn call_create_entry_remotely(agent: AgentPubKey) -> ExternResult<HeaderHash> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent.clone(),
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode()?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(WasmError::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            )))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(_) => call_create_entry_remotely(agent),
        ZomeCallResponse::CountersigningSession(e) => Err(WasmError::Guest(format!(
            "Countersigning session failed: {}",
            e
        ))),
    }
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

/// Same as above but doesn't recurse on network errors.
#[hdk_extern]
fn call_create_entry_remotely_no_rec(agent: AgentPubKey) -> ExternResult<HeaderHash> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent.clone(),
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode()?),
        ZomeCallResponse::Unauthorized(cell_id, zome_name, function_name, agent_pubkey) => {
            Err(WasmError::Guest(format!(
                "Unauthorized: {} {} {} {}",
                cell_id, zome_name, function_name, agent_pubkey
            )))
        }
        // Unbounded recursion.
        ZomeCallResponse::NetworkError(e) => Err(WasmError::Guest(format!("Network Error: {}", e))),
        ZomeCallResponse::CountersigningSession(e) => Err(WasmError::Guest(format!(
            "Countersigning session failed: {}",
            e
        ))),
    }
}
