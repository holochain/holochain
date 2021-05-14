use hdk::prelude::*;

#[hdk_entry(id = "foo", visibility = "public")]
#[derive(Clone)]
struct Foo(String);

entry_defs![Foo::entry_def()];

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to accept_cap_claim so other agents can send us claims
    let mut functions: GrantedFunctions = HashSet::new();
    functions.insert((zome_info()?.zome_name, "get_links_on_foo".into()));
    // functions.insert((zome_info!()?.zome_name, "needs_cap_claim".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn create_and_link_foo(_: ()) -> ExternResult<()> {
    let base = Foo("foo".to_string());
    let target = Foo("foofoo".to_string());
    let base_hash = hash_entry(&base)?;
    let target_hash = hash_entry(&base)?;
    let tag = LinkTag::new("foos");
    create_entry(&base)?;
    create_entry(&target)?;
    create_link(base_hash, target_hash, tag)?;
    Ok(())
}

#[hdk_extern]
fn get_links_on_foo(_: ()) -> ExternResult<Links> {
    let base = Foo("foo".to_string());
    let base_hash = hash_entry(&base)?;
    Ok(get_links(base_hash, None)?)
}
