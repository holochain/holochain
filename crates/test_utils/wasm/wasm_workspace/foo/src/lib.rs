use hdk::prelude::*;

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to accept_cap_claim so other agents can send us claims
    let mut fns = BTreeSet::new();
    fns.insert((zome_info()?.name, "foo".into()));
    // fns.insert((zome_info()?.name, "needs_cap_claim".into()));
    let functions = GrantedFunctions::Listed(fns);
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn foo(_: ()) -> ExternResult<String> {
    Ok(String::from("foo"))
}
