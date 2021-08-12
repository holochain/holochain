use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn create_a_thing(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing)
}

#[hdk_extern]
fn create_a_countersigned_thing(responses: Vec<PreflightResponse>) -> ExternResult<HeaderHash> {
    HDK.with(|h| h.borrow().create(EntryWithDefId::new(
        (&Thing).into(),
        Entry::CounterSign(
            Box::new(CounterSigningSessionData::try_from_responses(responses).map_err(|countersigning_error| WasmError::Guest(countersigning_error.to_string()))?),
            Thing.try_into()?,
        )
    )))
}

#[hdk_extern]
fn generate_countersigning_preflight_request(agents: Vec<(AgentPubKey, Vec<Role>)>) -> ExternResult<PreflightRequest> {
    PreflightRequest::try_new(
        agents,
        None,
        session_times_from_millis(5000)?,
        HeaderBase::Create(CreateBase::new(entry_type!(Thing)?, hash_entry(Thing)?)),
        PreflightBytes(vec![]),
    ).map_err(|e| WasmError::Guest(e.to_string()))
}

#[hdk_extern]
fn accept_countersigning_preflight_request(preflight_request: PreflightRequest) -> ExternResult<PreflightRequestAcceptance> {
    hdk::prelude::accept_countersigning_preflight_request(preflight_request)
}