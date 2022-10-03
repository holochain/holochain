use holochain_diagnostics::holochain::prelude::*;

pub fn basic_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique([EntryDef::from_id("a")], 1)
        .function(
            "create",
            |api, (base, bytes): (AnyLinkableHash, Vec<u8>)| {
                let entry: SerializedBytes = UnsafeBytes::from(bytes).try_into().unwrap();
                let hash = api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                    EntryVisibility::Public,
                    Entry::App(AppEntryBytes(entry)),
                    ChainTopOrdering::default(),
                ))?;
                let _ = api.create_link(CreateLinkInput::new(
                    base,
                    hash.clone().into(),
                    ZomeId(0),
                    LinkType::new(0),
                    ().into(),
                    ChainTopOrdering::default(),
                ))?;
                Ok(hash)
            },
        )
        .function("link_count", |api, base: AnyLinkableHash| {
            let links = api
                .get_links(vec![GetLinksInput::new(
                    base,
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
