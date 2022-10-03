fn main() {
    println!("Hello, world!");
}

use holochain_diagnostics::holochain::prelude::*;

pub fn basic_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique([EntryDef::from_id("a")], 1)
        .function("create", |api, (agent, bytes): (AgentPubKey, Vec<u8>)| {
            let entry: SerializedBytes = UnsafeBytes::from(bytes).try_into().unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::App(AppEntryBytes(entry)),
                ChainTopOrdering::default(),
            ))?;
            let _ = api.create_link(CreateLinkInput::new(
                agent.into(),
                hash.clone().into(),
                ZomeId(0),
                LinkType::new(0),
                ().into(),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("link_count", |api, agent: AgentPubKey| {
            let links = api
                .get_links(vec![GetLinksInput::new(
                    agent.into(),
                    LinkTypeFilter::single_dep(0.into()),
                    None,
                )])
                .unwrap();
            let links = links.first().unwrap();
            let gets = links
                .iter()
                .map(|l| {
                    let target = l.target.clone().retype(holo_hash::hash_type::Action);
                    GetInput::new(target.into(), Default::default())
                })
                .collect();
            let somes = api
                .get(gets)
                .unwrap()
                .into_iter()
                .filter(|e| e.is_some())
                .count();
            Ok(somes)
        })
        .function("validate", |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        })
}
