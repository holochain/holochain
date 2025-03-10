use crate::integrity::*;
use hdk::prelude::*;

const STANDARD_TIMEOUT_MILLIS: u64 = 30000;
const FAST_TIMEOUT_MILLIS: u64 = 10000;

#[hdk_extern]
fn create_a_thing(_: ()) -> ExternResult<ActionHash> {
    create_entry(&EntryTypes::Thing(Thing::Valid))
}

#[hdk_extern]
fn create_an_invalid_thing(_: ()) -> ExternResult<ActionHash> {
    create_entry(&EntryTypes::Thing(Thing::Invalid))
}

fn create_countersigned(
    responses: Vec<PreflightResponse>,
    thing: Thing,
) -> ExternResult<(ActionHash, EntryHash)> {
    let thing = EntryTypes::Thing(thing);
    let entry_def_index = ScopedEntryDefIndex::try_from(&thing)?;
    let visibility = EntryVisibility::from(&thing);

    let thing = match thing {
        EntryTypes::Thing(t) => t,
    };

    let entry = Entry::CounterSign(
        Box::new(
            CounterSigningSessionData::try_from_responses(responses, vec![]).map_err(
                |countersigning_error| {
                    wasm_error!(WasmErrorInner::Guest(countersigning_error.to_string()))
                },
            )?,
        ),
        thing.try_into()?,
    );
    let action_hash: ActionHash = HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            entry_def_index,
            visibility,
            entry,
            // Countersigned entries MUST have strict ordering.
            ChainTopOrdering::Strict,
        ))
    })?;

    let signed_action: SignedActionHashed = must_get_action(action_hash.clone())?;
    let entry_hash: EntryHash = signed_action.action().entry_hash().unwrap().clone();

    Ok((action_hash, entry_hash))
}

#[hdk_extern]
fn create_an_invalid_countersigned_thing(
    responses: Vec<PreflightResponse>,
) -> ExternResult<ActionHash> {
    Ok(create_countersigned(responses, Thing::Invalid)?.0)
}

#[hdk_extern]
fn create_a_countersigned_thing(responses: Vec<PreflightResponse>) -> ExternResult<ActionHash> {
    Ok(create_countersigned(responses, Thing::Valid)?.0)
}

#[hdk_extern]
fn create_a_countersigned_thing_with_entry_hash(
    responses: Vec<PreflightResponse>,
) -> ExternResult<(ActionHash, EntryHash)> {
    create_countersigned(responses, Thing::Valid)
}

fn generate_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
    thing: Thing,
    enzymatic: bool,
    session_timeout: u64,
) -> ExternResult<PreflightRequest> {
    let hash = hash_entry(&thing)?;
    let thing = EntryTypes::Thing(thing);
    let entry_type = thing.try_into()?;
    PreflightRequest::try_new(
        hash,
        agents,
        vec![],
        0,
        enzymatic,
        session_times_from_millis(session_timeout)?,
        ActionBase::Create(CreateBase::new(entry_type)),
        PreflightBytes(vec![]),
    )
    .map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))
}

#[hdk_extern]
fn generate_countersigning_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Valid, false, STANDARD_TIMEOUT_MILLIS)
}

#[hdk_extern]
fn generate_countersigning_preflight_request_fast(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Valid, false, FAST_TIMEOUT_MILLIS)
}

#[hdk_extern]
fn generate_countersigning_preflight_request_enzymatic(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Valid, true, STANDARD_TIMEOUT_MILLIS)
}

#[hdk_extern]
fn generate_invalid_countersigning_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Invalid, false, STANDARD_TIMEOUT_MILLIS)
}

#[hdk_extern]
fn generate_invalid_countersigning_preflight_request_enzymatic(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Invalid, true, STANDARD_TIMEOUT_MILLIS)
}

#[hdk_extern]
fn accept_countersigning_preflight_request(
    preflight_request: PreflightRequest,
) -> ExternResult<PreflightRequestAcceptance> {
    hdk::prelude::accept_countersigning_preflight_request(preflight_request)
}

#[hdk_extern]
fn must_get_action(action_hash: ActionHash) -> ExternResult<SignedActionHashed> {
    hdk::prelude::must_get_action(action_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}

#[hdk_extern]
fn must_get_valid_record(action_hash: ActionHash) -> ExternResult<Record> {
    hdk::prelude::must_get_valid_record(action_hash)
}

#[hdk_extern]
fn get_agent_activity(input: GetAgentActivityInput) -> ExternResult<AgentActivity> {
    HDK.with(|h| h.borrow().get_agent_activity(input))
}

#[cfg(feature = "unstable-functions")]
#[hdk_extern]
fn schedule_signal() -> ExternResult<()> {
    HDK.with(|h| h.borrow().schedule("scheduled_fn".to_string()))
}

#[cfg(feature = "unstable-functions")]
#[hdk_extern(infallible)]
fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    emit_signal("scheduled hello");
    Some(Schedule::from("*/1 * * * * *".to_string()))
}
