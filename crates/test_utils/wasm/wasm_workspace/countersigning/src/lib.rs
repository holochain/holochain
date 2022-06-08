use hdk::prelude::*;

#[hdk_entry(id = "thing")]
enum Thing {
    Valid,
    Invalid,
}

impl From<Thing> for ValidateCallbackResult {
    fn from(thing: Thing) -> Self {
        match thing {
            Thing::Valid => ValidateCallbackResult::Valid,
            Thing::Invalid => ValidateCallbackResult::Invalid("never valid".to_string()),
        }
    }
}

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    let this_zome = zome_info()?;
    match op {
        Op::StoreElement { element }
            if element.header().entry_type().map_or(false, |et| match et {
                EntryType::App(app_entry_type) => {
                    this_zome.matches_entry_def_id(app_entry_type, Thing::entry_def_id())
                }
                _ => false,
            }) =>
        {
            Ok(Thing::try_from(element)?.into())
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

#[hdk_extern]
fn create_a_thing(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing::Valid)
}

#[hdk_extern]
fn create_an_invalid_thing(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing::Invalid)
}

fn create_countersigned(
    responses: Vec<PreflightResponse>,
    thing: Thing,
) -> ExternResult<HeaderHash> {
    HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            (&thing).into(),
            Entry::CounterSign(
                Box::new(
                    CounterSigningSessionData::try_from_responses(responses).map_err(
                        |countersigning_error| WasmError::Guest(countersigning_error.to_string()),
                    )?,
                ),
                thing.try_into()?,
            ),
            // Countersigned entries MUST have strict ordering.
            ChainTopOrdering::Strict,
        ))
    })
}

#[hdk_extern]
fn create_an_invalid_countersigned_thing(
    responses: Vec<PreflightResponse>,
) -> ExternResult<HeaderHash> {
    create_countersigned(responses, Thing::Invalid)
}

#[hdk_extern]
fn create_a_countersigned_thing(responses: Vec<PreflightResponse>) -> ExternResult<HeaderHash> {
    create_countersigned(responses, Thing::Valid)
}

fn generate_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
    thing: Thing,
) -> ExternResult<PreflightRequest> {
    PreflightRequest::try_new(
        hash_entry(thing)?,
        agents,
        None,
        session_times_from_millis(5000)?,
        HeaderBase::Create(CreateBase::new(entry_type!(Thing)?, Default::default())),
        PreflightBytes(vec![]),
    )
    .map_err(|e| WasmError::Guest(e.to_string()))
}

#[hdk_extern]
fn generate_countersigning_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Valid)
}

#[hdk_extern]
fn generate_invalid_countersigning_preflight_request(
    agents: Vec<(AgentPubKey, Vec<Role>)>,
) -> ExternResult<PreflightRequest> {
    generate_preflight_request(agents, Thing::Invalid)
}

#[hdk_extern]
fn accept_countersigning_preflight_request(
    preflight_request: PreflightRequest,
) -> ExternResult<PreflightRequestAcceptance> {
    hdk::prelude::accept_countersigning_preflight_request(preflight_request)
}

#[hdk_extern]
fn must_get_header(header_hash: HeaderHash) -> ExternResult<SignedHeaderHashed> {
    hdk::prelude::must_get_header(header_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn get_agent_activity(input: GetAgentActivityInput) -> ExternResult<AgentActivity> {
    HDK.with(|h| h.borrow().get_agent_activity(input))
}
