use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.name, "ping".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

#[hdk_extern]
fn ping(agent: AgentPubKey) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::Ping(Ping(agent)))
}

#[hdk_extern(infallible)]
fn post_commit(shhs: Vec<SignedActionHashed>) {
    if let Ok(ping) =
        Ping::try_from(must_get_entry(shhs[0].action().entry_hash().unwrap().clone()).unwrap())
    {
        if hdk::prelude::query(
            ChainQueryFilter::default().entry_type(EntryTypesUnit::Ping.try_into().unwrap()),
        )
        .unwrap()
        .len()
            < PINGS
        {
            call_remote(
                ping.0,
                zome_info().unwrap().name,
                "ping".to_string().into(),
                None,
                &agent_info().unwrap().agent_latest_pubkey,
            )
            .unwrap();
        }
    }
}

#[hdk_extern]
fn query(_: ()) -> ExternResult<Vec<Commit>> {
    hdk::prelude::query(ChainQueryFilter::default().entry_type(EntryTypesUnit::Ping.try_into()?))
}
