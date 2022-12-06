use std::collections::BTreeSet;

use holochain_diagnostics::{dht::test_utils::seeded_rng, holochain::prelude::*, random_bytes};

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
                    ChainTopOrdering::Relaxed,
                ))?;
                let _ = api.create_link(CreateLinkInput::new(
                    base,
                    hash.clone().into(),
                    ZomeIndex(0),
                    LinkType::new(0),
                    ().into(),
                    ChainTopOrdering::Relaxed,
                ))?;
                Ok(hash)
            },
        )
        .function(
            "create_batch_random",
            |api, (base, num, size): (AnyLinkableHash, u32, u32)| {
                let mut rng = seeded_rng(None);
                for _ in 0..num {
                    let bytes = random_bytes(&mut rng, size as usize);
                    let entry: SerializedBytes =
                        UnsafeBytes::from(bytes.into_vec()).try_into().unwrap();
                    let hash = api.create(CreateInput::new(
                        InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                        EntryVisibility::Public,
                        Entry::App(AppEntryBytes(entry)),
                        ChainTopOrdering::Relaxed,
                    ))?;
                    let _ = api.create_link(CreateLinkInput::new(
                        base.clone(),
                        hash.clone().into(),
                        ZomeIndex(0),
                        LinkType::new(0),
                        ().into(),
                        ChainTopOrdering::Relaxed,
                    ))?;
                }
                Ok(())
            },
        )
        .function(
            "link_count",
            |api, (base, entries): (AnyLinkableHash, bool)| {
                let links = api
                    .get_links(vec![GetLinksInput::new(
                        base,
                        LinkTypeFilter::single_dep(0.into()),
                        None,
                    )])
                    .unwrap();
                let links = links.first().unwrap();
                if entries {
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
                } else {
                    Ok(links.len())
                }
            },
        )
        .function("validate", |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        })
}

pub fn syn_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique([EntryDef::from_id("a")], 0)
        .function("commit", |api, bytes: Vec<u8>| {
            let entry: SerializedBytes = UnsafeBytes::from(bytes).try_into().unwrap();
            api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::App(AppEntryBytes(entry)),
                ChainTopOrdering::Relaxed,
            ))?;
            Ok(())
        })
        //
        .function(
            "send_message",
            |api, (msg, agents): (Vec<u8>, Vec<AgentPubKey>)| {
                api.remote_signal(RemoteSignal {
                    agents,
                    signal: ExternIO::encode(msg).unwrap(),
                })?;

                // let res = api.call(vec![Call {
                //     target: CallTarget::NetworkAgent(agents[1].clone()),
                //     zome_name: "zome".into(),
                //     fn_name: "recv_remote_signal".into(),
                //     cap_secret: None,
                //     payload: msg.into(),
                // }])?;
                // dbg!(res);
                Ok(())
            },
        )
        //
        .function("recv_remote_signal", |api, signal: ExternIO| {
            Ok(api.emit_signal(AppSignal::new(signal))?)
        })
        //
        .function("init", move |api, ()| {
            let mut functions: GrantedFunctions = BTreeSet::new();
            functions.insert((api.zome_info(()).unwrap().name, "recv_remote_signal".into()));
            let cap_grant_entry = CapGrantEntry {
                tag: "".into(),
                // empty access converts to unrestricted
                access: ().into(),
                functions,
            };
            api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(cap_grant_entry),
                ChainTopOrdering::Relaxed,
            ))
            .unwrap();

            Ok(InitCallbackResult::Pass)
        })
}
