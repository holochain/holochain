//! Facts about DhtOps

use crate::prelude::*;
use ::contrafact::*;
use holochain_keystore::MetaLairClient;
use holochain_zome_types::facts::ActionRefMut;

/// Fact: The ChainOp is internally consistent in all of its references:
/// - TODO: The ChainOp variant matches the Action variant
/// - The Signature matches the Action
/// - If the action references an Entry, the Entry will exist and be of the appropriate hash
/// - If the action does not reference an Entry, the entry will be None
pub fn valid_chain_op(
    keystore: MetaLairClient,
    author: AgentPubKey,
    must_be_public: bool,
) -> impl Fact<'static, ChainOp> {
    facts![
        brute(
            "Action type matches Entry existence, and is public if exists",
            move |op: &ChainOp| {
                let action = op.action();
                let h = action.entry_data();
                let e = op.entry();
                match (h, e) {
                    (
                        Some((_entry_hash, entry_type)),
                        RecordEntry::Present(_) | RecordEntry::NotStored,
                    ) => {
                        // Ensure that entries are public
                        !must_be_public || entry_type.visibility().is_public()
                    }
                    (None, RecordEntry::Present(_)) => false,
                    (None, _) => true,
                    _ => false,
                }
            }
        ),
        lambda_unit(
            "If there is entry data, the action must point to it",
            |g, op: ChainOp| {
                if let Some(entry) = op.entry().into_option() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    prism(
                        "action's entry hash",
                        |op: &mut ChainOp| op.action_entry_data_mut().map(|(hash, _)| hash),
                        eq(EntryHash::with_data_sync(entry)),
                    )
                    .mutate(g, op)
                } else {
                    Ok(op)
                }
            }
        ),
        lens1(
            "The author is the one specified",
            ChainOp::author_mut,
            eq(author)
        ),
        lambda_unit("The Signature matches the Action", move |g, op: ChainOp| {
            use holochain_keystore::AgentPubKeyExt;
            let action = op.action();
            let agent = action.author();
            let actual = tokio_helper::block_forever_on(agent.sign(&keystore, &action))
                .expect("Can sign the action");
            lens1("signature", ChainOp::signature_mut, eq(actual)).mutate(g, op)
        })
    ]
}

impl ChainOp {
    /// Mutable access to the Author
    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            ChainOp::StoreRecord(_, h, _) => h.author_mut(),
            ChainOp::StoreEntry(_, h, _) => h.author_mut(),
            ChainOp::RegisterAgentActivity(_, h) => h.author_mut(),
            ChainOp::RegisterUpdatedContent(_, h, _) => &mut h.author,
            ChainOp::RegisterUpdatedRecord(_, h, _) => &mut h.author,
            ChainOp::RegisterDeletedBy(_, h) => &mut h.author,
            ChainOp::RegisterDeletedEntryAction(_, h) => &mut h.author,
            ChainOp::RegisterAddLink(_, h) => &mut h.author,
            ChainOp::RegisterRemoveLink(_, h) => &mut h.author,
        }
    }

    /// Mutable access to the Timestamp
    pub fn timestamp_mut(&mut self) -> &mut Timestamp {
        match self {
            ChainOp::StoreRecord(_, h, _) => h.timestamp_mut(),
            ChainOp::StoreEntry(_, h, _) => h.timestamp_mut(),
            ChainOp::RegisterAgentActivity(_, h) => h.timestamp_mut(),
            ChainOp::RegisterUpdatedContent(_, h, _) => &mut h.timestamp,
            ChainOp::RegisterUpdatedRecord(_, h, _) => &mut h.timestamp,
            ChainOp::RegisterDeletedBy(_, h) => &mut h.timestamp,
            ChainOp::RegisterDeletedEntryAction(_, h) => &mut h.timestamp,
            ChainOp::RegisterAddLink(_, h) => &mut h.timestamp,
            ChainOp::RegisterRemoveLink(_, h) => &mut h.timestamp,
        }
    }

    /// Mutable access to the Signature
    pub fn signature_mut(&mut self) -> &mut Signature {
        match self {
            ChainOp::StoreRecord(s, _, _) => s,
            ChainOp::StoreEntry(s, _, _) => s,
            ChainOp::RegisterAgentActivity(s, _) => s,
            ChainOp::RegisterUpdatedContent(s, _, _) => s,
            ChainOp::RegisterUpdatedRecord(s, _, _) => s,
            ChainOp::RegisterDeletedBy(s, _) => s,
            ChainOp::RegisterDeletedEntryAction(s, _) => s,
            ChainOp::RegisterAddLink(s, _) => s,
            ChainOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Mutable access to the seq of the Action, if applicable
    pub fn action_seq_mut(&mut self) -> Option<&mut u32> {
        match self {
            ChainOp::StoreRecord(_, ref mut h, _) => h.action_seq_mut(),
            ChainOp::StoreEntry(_, ref mut h, _) => h.action_seq_mut(),
            ChainOp::RegisterAgentActivity(_, ref mut h) => h.action_seq_mut(),
            ChainOp::RegisterUpdatedContent(_, ref mut h, _) => Some(&mut h.action_seq),
            ChainOp::RegisterUpdatedRecord(_, ref mut h, _) => Some(&mut h.action_seq),
            ChainOp::RegisterDeletedBy(_, ref mut h) => Some(&mut h.action_seq),
            ChainOp::RegisterDeletedEntryAction(_, ref mut h) => Some(&mut h.action_seq),
            ChainOp::RegisterAddLink(_, ref mut h) => Some(&mut h.action_seq),
            ChainOp::RegisterRemoveLink(_, ref mut h) => Some(&mut h.action_seq),
        }
    }

    /// Mutable access to the entry data of the Action, if applicable
    pub fn action_entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            ChainOp::StoreRecord(_, ref mut h, _) => h.entry_data_mut(),
            ChainOp::StoreEntry(_, ref mut h, _) => h.entry_data_mut(),
            ChainOp::RegisterAgentActivity(_, ref mut h) => h.entry_data_mut(),
            ChainOp::RegisterUpdatedContent(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            ChainOp::RegisterUpdatedRecord(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            _ => None,
        }
    }
}

impl ActionRefMut for NewEntryAction {
    fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            Self::Create(Create { ref mut author, .. }) => author,
            Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    fn action_seq_mut(&mut self) -> Option<&mut u32> {
        Some(match self {
            Self::Create(Create {
                ref mut action_seq, ..
            }) => action_seq,
            Self::Update(Update {
                ref mut action_seq, ..
            }) => action_seq,
        })
    }

    fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        todo!()
    }

    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        Some(match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
        })
    }

    fn timestamp_mut(&mut self) -> &mut Timestamp {
        match self {
            Self::Create(Create {
                ref mut timestamp, ..
            }) => timestamp,
            Self::Update(Update {
                ref mut timestamp, ..
            }) => timestamp,
        }
    }
}

impl NewEntryAction {
    /// Mutable access to the entry hash
    pub fn entry_hash_mut(&mut self) -> &mut EntryHash {
        self.entry_data_mut().unwrap().0
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::Arbitrary;

    use super::*;
    use holochain_zome_types::facts;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_valid_dht_op() {
        // TODO: Must add constraint on dht op variant wrt action variant

        let mut gg = Generator::from(unstructured_noise());
        let g = &mut gg;
        let keystore = holochain_keystore::spawn_test_keystore().await.unwrap();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();

        let e = Entry::arbitrary(g).unwrap();

        let mut a0 = facts::is_not_entry_action().build(g);
        *a0.author_mut() = agent.clone();

        let mut a1 = facts::is_new_entry_action().build(g);
        *a1.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        *a1.author_mut() = agent.clone();

        let sn = agent.sign(&keystore, &a0).await.unwrap();
        let se = agent.sign(&keystore, &a1).await.unwrap();

        let op0a = ChainOp::StoreRecord(sn.clone(), a0.clone(), RecordEntry::Present(e.clone()));
        let op0b = ChainOp::StoreRecord(sn.clone(), a0.clone(), RecordEntry::Hidden);
        let op0c = ChainOp::StoreRecord(sn.clone(), a0.clone(), RecordEntry::NA);
        let op0d = ChainOp::StoreRecord(sn.clone(), a0.clone(), RecordEntry::NotStored);

        let op1a = ChainOp::StoreRecord(se.clone(), a1.clone(), RecordEntry::Present(e.clone()));
        let op1b = ChainOp::StoreRecord(se.clone(), a1.clone(), RecordEntry::Hidden);
        let op1c = ChainOp::StoreRecord(se.clone(), a1.clone(), RecordEntry::NA);
        let op1d = ChainOp::StoreRecord(se.clone(), a1.clone(), RecordEntry::NotStored);

        let fact = valid_chain_op(keystore, agent, false);

        assert!(fact.clone().check(&op0a).is_err());
        fact.clone().check(&op0b).unwrap();
        fact.clone().check(&op0c).unwrap();
        fact.clone().check(&op0d).unwrap();

        fact.clone().check(&op1a).unwrap();
        assert!(fact.clone().check(&op1b).is_err());
        assert!(fact.clone().check(&op1c).is_err());
        fact.clone().check(&op1d).unwrap();
    }
}
