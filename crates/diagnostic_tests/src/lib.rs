use std::time::Instant;

use holochain_diagnostics::holochain::conductor::config::ConductorConfig;
use holochain_diagnostics::holochain::prelude::*;
use holochain_diagnostics::holochain::sweettest::*;

pub async fn setup_conductors_single_zome(
    nodes: usize,
    config: ConductorConfig,
    zome: InlineIntegrityZome,
) -> (SweetConductorBatch, Vec<SweetZome>) {
    let config = standard_config();

    let start = Instant::now();

    let mut conductors = SweetConductorBatch::from_config(nodes, config).await;
    println!("Conductors created (t={:3.1?}).", start.elapsed());

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome", zome)).await;
    let apps = conductors.setup_app("basic", &[dna]).await.unwrap();
    let cells = apps.cells_flattened().clone();
    println!("Apps setup (t={:3.1?}).", start.elapsed());

    let zomes = cells.iter().map(|c| c.zome("zome")).collect::<Vec<_>>();

    (conductors, zomes)
}

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

pub fn syn_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique([EntryDef::from_id("a")], 0)
        .function(
            "send_message",
            |api, (msg, agents): (Vec<u8>, Vec<AgentPubKey>)| {
                let signal = msg.into();
                Ok(api.remote_signal(RemoteSignal { agents, signal })?)
            },
        )
        .function("commit", |api, bytes: Vec<u8>| {
            let entry: SerializedBytes = UnsafeBytes::from(bytes).try_into().unwrap();
            api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::App(AppEntryBytes(entry)),
                ChainTopOrdering::default(),
            ))?;
            Ok(())
        })
        .function("recv_remote_signal", |api, signal| {
            Ok(api.emit_signal(signal)?)
        })
}
