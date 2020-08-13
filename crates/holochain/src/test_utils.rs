use ::fixt::prelude::*;
use holo_hash::fixt::*;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::HolochainP2pRefToCell, event::HolochainP2pEventReceiver, spawn_holochain_p2p,
    HolochainP2pCell, HolochainP2pRef, HolochainP2pSender,
};
use holochain_serialized_bytes::UnsafeBytes;
use holochain_types::{
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    test_utils::fake_header_hash,
    Entry, EntryHashed, HeaderHashed, Timestamp,
};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::header::{EntryCreate, EntryType, Header};
use std::convert::TryInto;

#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

/// Create a fake SignedHeaderHashed and EntryHashed pair with random content
pub async fn fake_unique_element(
    keystore: &KeystoreSender,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedHeaderHashed, EntryHashed)> {
    let content = UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned());
    let entry = EntryHashed::from_content(Entry::App(content.try_into().unwrap())).await;
    let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
        .next()
        .unwrap();
    let header_1 = Header::EntryCreate(EntryCreate {
        author: agent_key,
        timestamp: Timestamp::now().into(),
        header_seq: 0,
        prev_header: fake_header_hash(1),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, HeaderHashed::from_content(header_1).await).await?,
        entry,
    ))
}

/// Convenience constructor for cell networks
pub async fn test_network(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
) -> (HolochainP2pRef, HolochainP2pEventReceiver, HolochainP2pCell) {
    let (network, recv) = spawn_holochain_p2p().await.unwrap();
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna.clone(), agent_key).await.unwrap();
    (network, recv, cell_network)
}
