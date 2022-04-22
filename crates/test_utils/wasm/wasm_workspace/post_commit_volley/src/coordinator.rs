use crate::integrity::*;
use hdk::prelude::*;

#[hdk_entry_zomes]
enum EntryZomes {
    IntegrityPostCommitVolly(EntryTypes),
}

#[derive(ToZomeName)]
enum Zomes {
    IntegrityPostCommitVolly,
}

impl EntryZomes {
    fn ping(ping: Ping) -> Self {
        EntryZomes::IntegrityPostCommitVolly(EntryTypes::Ping(ping))
    }
}

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
fn ping(agent: AgentPubKey) -> ExternResult<HeaderHash> {
    create_entry(EntryZomes::ping(Ping(agent)))
}

#[hdk_extern(infallible)]
fn post_commit(shhs: Vec<SignedHeaderHashed>) {
    if let Ok(ping) =
        Ping::try_from(must_get_entry(shhs[0].header().entry_hash().unwrap().clone()).unwrap())
    {
        if hdk::prelude::query(
            ChainQueryFilter::default()
                .entry_type(EntryZomes::ping(ping.clone()).entry_type().unwrap()),
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
fn query(_: ()) -> ExternResult<Vec<Element>> {
    let DnaInfo { zome_names, .. } = dna_info()?;
    let zome_name: ZomeName = Zomes::IntegrityPostCommitVolly.into();
    let zome_id = zome_names
        .iter()
        .position(|name| *name == zome_name)
        .map(|i| ZomeId(i as u8))
        .unwrap();

    let entry_type = EntryType::App(AppEntryType {
        id: EntryTypes::variant_to_entry_def_index(EntryTypes::Ping),
        zome_id,
        visibility: EntryTypes::variant_to_entry_visibility(EntryTypes::Ping),
    });
    hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type))
}
