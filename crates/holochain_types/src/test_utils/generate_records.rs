use crate::prelude::*;
use contrafact::Fact;
use holo_hash::{AgentPubKey, EntryHash};
use holochain_keystore::MetaLairClient;

/// Generate a chain of Records which constitutes a valid source chain:
/// - Each action will refer to the one prior
/// - Each Record will have a valid signature
pub async fn valid_arbitrary_chain<'a>(
    g: &mut contrafact::Generator<'a>,
    keystore: MetaLairClient,
    author: AgentPubKey,
    n: usize,
) -> Vec<Record> {
    let fact = contrafact::facts![
        holochain_zome_types::facts::action_and_entry_match(false),
        contrafact::lens1(
            "action is valid",
            |(a, _)| a,
            holochain_zome_types::facts::valid_chain_action(author.clone()),
        ),
    ];

    let pairs = contrafact::vec_of_length(n, fact).build(g);
    let chain: Vec<Record> = futures::future::join_all(pairs.into_iter().map(|(a, entry)| {
        let keystore = keystore.clone();
        assert_eq!(a.author(), &author);
        async move {
            Record::new(
                SignedActionHashed::sign(&keystore, ActionHashed::from_content_sync(a))
                    .await
                    .unwrap(),
                entry.into_option(),
            )
        }
    }))
    .await;

    chain
}

/// Get a signed Record from Action and Entry
pub async fn sign_record(
    keystore: &MetaLairClient,
    action: Action,
    entry: Option<Entry>,
) -> Record {
    Record::new(
        SignedActionHashed::sign(keystore, ActionHashed::from_content_sync(action))
            .await
            .unwrap(),
        entry,
    )
}

/// Reconstruct a record which has been modified so that it's valid
pub async fn rebuild_record(record: Record, keystore: &MetaLairClient) -> Record {
    let (action, entry) = record.into_inner();
    let mut action = action.into_inner().0.into_content();
    if let (Some(ed), Some(entry)) = (action.entry_data_mut(), entry.as_option()) {
        *ed.0 = EntryHash::with_data_sync(entry);
    }
    sign_record(keystore, action, entry.into_option()).await
}
