use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((hdk::prelude::zome_info()?.name, "call_info".into()));
    functions.insert((hdk::prelude::zome_info()?.name, "remote_call_info".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

#[hdk_extern]
fn call_info(_: ()) -> ExternResult<CallInfo> {
    // Commit something here so we can show the as_at won't shift in the call
    // info returned.
    create_entry(Thing)?;
    hdk::prelude::call_info()
}

#[hdk_extern]
fn remote_call_info(agent: AgentPubKey) -> ExternResult<CallInfo> {
    match call_remote(
        agent,
        hdk::prelude::zome_info()?.name,
        "call_info".to_string().into(),
        None,
        &(),
    )? {
        ZomeCallResponse::Ok(extern_io) => Ok(extern_io.decode()?),
        not_ok => {
            tracing::warn!(?not_ok);
            Err(WasmError::Guest(format!("{:?}", not_ok)))
        },
    }
}

#[hdk_extern]
fn remote_remote_call_info(agent: AgentPubKey) -> ExternResult<CallInfo> {
    match call_remote(
        agent,
        hdk::prelude::zome_info()?.name,
        "remote_call_info".to_string().into(),
        None,
        agent_info()?.agent_initial_pubkey,
    )? {
        ZomeCallResponse::Ok(extern_io) => Ok(extern_io.decode()?),
        not_ok => {
            tracing::warn!(?not_ok);
            Err(WasmError::Guest(format!("{:?}", not_ok)))
        },
    }
}

#[hdk_extern]
fn dna_info(_: ()) -> ExternResult<DnaInfo> {
    hdk::prelude::dna_info()
}

#[hdk_extern]
fn dna_info_foo(_: ()) -> ExternResult<serde_yaml::Value> {
    Ok(hdk::prelude::dna_info()?.properties["foo"].clone())
}

#[cfg(test)]
pub mod tests {
    use hdk::prelude::*;
    use ::fixt::prelude::*;

    #[test]
    fn zome_info_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let output = fixt!(ZomeInfo);
        let output_closure = output.clone();
        mock_hdk.expect_zome_info()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::zome_info(());

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}