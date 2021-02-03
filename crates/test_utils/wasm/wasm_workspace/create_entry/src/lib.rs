use hdk3::prelude::*;

#[hdk_entry(
    id = "setup",
    required_validations = 5,
    required_validation_type = "element"
)]
struct Setup(String);

#[hdk_entry(
    id = "post",
    required_validations = 5,
    required_validation_type = "full"
)]
#[derive(Debug)]
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

entry_defs![Post::entry_def(), Msg::entry_def(), PrivMsg::entry_def()];

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
    Ok(hdk3::prelude::create_entry(&post())?)
}

#[hdk_extern]
fn create_post(post: crate::Post) -> ExternResult<HeaderHash> {
    hdk3::prelude::create_entry(&post)
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
    // Creating multiple entries in a Zome function should be fine, but presently fails...
    // Run "cargo test validation" to trigger this to fail in get_validation_package_test
    //hdk3::prelude::create_entry(&Setup(String::from("Hello, before Msg...")))?;
    hdk3::prelude::create_entry(&msg())
}

#[hdk_extern]
fn create_priv_msg(_: ()) -> ExternResult<HeaderHash> {
    hdk3::prelude::create_entry(&priv_msg())
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

    // Test that the init function can also successfully commit entries to the source-chain.  Until
    // https://github.com/holochain/holochain/pull/601 is fixed, this will cause failure in test
    // cases ...::wasm_test::bridge_call and call_the_same_cell!  It appears that *any* Zome API
    // function that commits more than one Entry will fail (see below, in fn call_create_entry).
    // Run "cargo test wasm" to trigger this failure.
    //hdk3::prelude::create_entry(&Setup(String::from("Hello, world!")))?;

    Ok(InitCallbackResult::Pass)
}

/// Create a post entry then
/// create another post through a
/// call
#[hdk_extern]
fn call_create_entry(_: ()) -> ExternResult<HeaderHash> {
    // Creating multiple entries in a Zome function should be fine, but presently fails...
    // Run "cargo test wasm" to trigger this failure.
    hdk3::prelude::create_entry(&Setup(String::from("Hello, before Post...")))?;
    // Create an entry directly via. the hdk.
    hdk3::prelude::create_entry(&post())?;
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
