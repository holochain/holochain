use hdk::prelude::*;

#[hdk_entry(
    id = "setup",
    required_validations = 5,
    required_validation_type = "element"
)]
struct Setup(String);

/// An entry that only validates if the Entry's Timestamp matches the Header Timestamp.  This is
/// impossible, unless we can specify the Header Timestamp used for source-chain Element create,
/// via hdk::prelude::create with EntryWithDefId.at(<Timestamp>).
#[hdk_entry(
    id = "secs",
    required_validations = 5,
    required_validation_type = "full"
)]
struct Secs(Timestamp);

/// Construct an EntryDefWithId from Secs with the same Timestamp as the Header will have.
fn secs_now() -> ExternResult<EntryWithDefId> {
    let now: Timestamp = (Timestamp::epoch() + hdk::prelude::sys_time()?)
	.map_err(|e| WasmError::Guest(format!("Timestamp error: {}", e)))?;
    let secs = Secs(now);
    let entry_def_with_id: EntryWithDefId = secs.try_into()?;
    Ok(entry_def_with_id.at(now)) // And, specify a desired Header Timestamp!
}

#[hdk_entry(
    id = "post",
    required_validations = 5,
    required_validation_type = "full"
)]
struct Post(String);

#[hdk_entry(
    id = "msg",
    required_validations = 5,
    required_validation_type = "sub_chain"
)]
struct Msg(String);

#[hdk_entry(
    id = "priv_msg",
    required_validations = 5,
    required_validation_type = "full",
    visibility = "private"
)]
struct PrivMsg(String);

entry_defs![
    Post::entry_def(), Msg::entry_def(), PrivMsg::entry_def(), Setup::entry_def(), Secs::entry_def()
];

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
    Ok(hdk::prelude::create_entry(&post())?)
}

#[hdk_extern]
fn create_post(post: crate::Post) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&post)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Element>> {
    get(
        hash_entry(&post())?,
        GetOptions::content(),
    )
}

#[hdk_extern]
fn get_post(hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(
        hash,
        GetOptions::content()
    )
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
fn validate_create_entry_post(
    validation_data: ValidateData,
) -> ExternResult<ValidateCallbackResult> {
    let element = validation_data.element;
    let r = match element.entry().to_app_option::<Post>() {
        Ok(Some(post)) if &post.0 == "Banana" => {
            ValidateCallbackResult::Invalid("No Bananas!".to_string())
        }
        _ => ValidateCallbackResult::Valid,
    };
    Ok(r)
}


#[hdk_extern]
fn validate_create_entry_secs(
    validation_data: ValidateData,
) -> ExternResult<ValidateCallbackResult> {
    let r = match validation_data.element.entry().to_app_option::<Secs>() {
        Ok(Some(secs)) => if secs.0 == validation_data.element.header().timestamp() {
	    ValidateCallbackResult::Valid // Header Timestamp matches Sec(Timestamp)!
        } else {
            ValidateCallbackResult::Invalid(format!(
		"Timestamp Mismatch: {:?} vs. {:?}", secs, validation_data.element.header() ))
	},
        other => ValidateCallbackResult::Invalid(format!("Not a Secs Entry: {:?}", other ))
    };
    Ok(r)
}


#[hdk_extern]
fn get_activity(
    input: holochain_test_wasm_common::AgentActivitySearch,
) -> ExternResult<AgentActivity> {
    get_agent_activity(
        input.agent,
        input.query,
        input.request
    )
}

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to create_entry Zome API so other agents can create entries
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.zome_name, "create_entry".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;
    // Test that the init function can also successfully commit multiple entries to the source-chain
    hdk::prelude::create_entry(&Setup(String::from("Hello, world!")))?;
    Ok(InitCallbackResult::Pass)
}

/// Create a post entry then
/// create another post through a
/// call
#[hdk_extern]
fn call_create_entry(_: ()) -> ExternResult<HeaderHash> {
    // Creating multiple entries in a Zome function should also be fine.
    hdk::prelude::create_entry(&Setup(String::from("Hello, before Post...")))?;

    // Creating an entry with a custom EntryWithDefId timestamp should pose no issues
    hdk::prelude::create(secs_now()?)?;

    // Create an entry directly via. the hdk.
    hdk::prelude::create_entry(&post())?;
    // Create an entry via a `call`.
    let zome_call_response: ZomeCallResponse = call(
        None,
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode()?),
        // Should handle this in real code.
        _ => unreachable!(),
    }
}

#[hdk_extern]
fn call_create_entry_remotely(agent: AgentPubKey) -> ExternResult<HeaderHash> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent,
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?;

    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode()?),
        // Handle this in real code.
        _ => unreachable!(),
    }
}
