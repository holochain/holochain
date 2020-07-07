use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_serialized_bytes::UnsafeBytes;
use holochain_types::{
    element::SignedHeaderHashed,
    header::{EntryCreate, EntryType},
    test_utils::fake_header_hash,
    Entry, EntryHashed, Header, HeaderHashed, Timestamp,
};
use holochain_zome_types::entry_def::EntryVisibility;
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
    let entry = EntryHashed::with_data(Entry::App(content.try_into().unwrap())).await?;
    let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
        .next()
        .unwrap();
    let header_1 = Header::EntryCreate(EntryCreate {
        author: agent_key,
        timestamp: Timestamp::now(),
        header_seq: 0,
        prev_header: fake_header_hash("1"),

        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry.as_hash().to_owned(),
    });

    Ok((
        SignedHeaderHashed::new(&keystore, HeaderHashed::with_data(header_1).await?).await?,
        entry,
    ))
}
