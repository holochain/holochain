use crate::core::state::source_chain::SignedHeaderHashed;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_serialized_bytes::UnsafeBytes;
use holochain_types::{
    header::{EntryCreate, EntryType, EntryVisibility},
    test_utils::{fake_app_entry_type, fake_header_hash},
    Entry, EntryHashed, Header, HeaderHashed, Timestamp,
};
use std::convert::TryInto;

/// Create a fake SignedHeaderHashed and EntryHashed pair with random content
pub async fn fake_unique_element(
    keystore: &KeystoreSender,
    agent_key: AgentPubKey,
    visibility: EntryVisibility,
) -> anyhow::Result<(SignedHeaderHashed, EntryHashed)> {
    let content = UnsafeBytes::from(nanoid::nanoid!().as_bytes().to_owned());
    let entry = EntryHashed::with_data(Entry::App(content.try_into().unwrap())).await?;
    let header_1 = Header::EntryCreate(EntryCreate {
        author: agent_key,
        timestamp: Timestamp::now(),
        header_seq: 0,
        prev_header: fake_header_hash("1").into(),

        entry_type: EntryType::App(fake_app_entry_type(1, visibility)),
        entry_address: entry.as_hash().to_owned().into(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, HeaderHashed::with_data(header_1).await?).await?,
        entry,
    ))
}
