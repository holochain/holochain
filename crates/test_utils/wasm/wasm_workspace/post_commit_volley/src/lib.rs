use hdk::prelude::*;

const PINGS: usize = 5;

#[hdk_entry(id = "ping")]
struct Ping(AgentPubKey);

entry_defs![Ping::entry_def()];

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.zome_name, "ping".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

#[hdk_extern]
fn ping(agent: AgentPubKey) -> ExternResult<HeaderHash> {
    create_entry(Ping(agent))
}

#[hdk_extern(infallible)]
fn post_commit(shhs: Vec<SignedHeaderHashed>) {
    if let Ok(ping) = Ping::try_from(must_get_entry(shhs[0].header().entry_hash().unwrap().clone()).unwrap()) {
        if hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Ping).unwrap())).unwrap().len() < PINGS {
            call_remote(
                ping.0,
                zome_info().unwrap().zome_name,
                "ping".to_string().into(),
                None,
                &agent_info().unwrap().agent_latest_pubkey,
            ).unwrap();
        }
    }
}

#[hdk_extern]
fn query(_: ()) -> ExternResult<Vec<Element>> {
    hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Ping).unwrap()))
}