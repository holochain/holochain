//! Facts about DhtOps

use super::*;
use ::contrafact::*;
use holochain_keystore::MetaLairClient;

/// Fact: The DhtOp is internally consistent in all of its references:
/// - TODO: The DhtOp variant matches the Action variant
/// - The Signature matches the Action
/// - If the action references an Entry, the Entry will exist and be of the appropriate hash
/// - If the action does not reference an Entry, the entry will be None
pub fn valid_dht_op(keystore: MetaLairClient) -> Facts<'static, DhtOp> {
    facts![
        brute("Action type matches Entry existence", |op: &DhtOp| {
            let has_action = op.action().entry_data().is_some();
            let has_entry = op.entry().is_some();
            has_action == has_entry
        }),
        mapped(
            "If there is entry data, the action must point to it",
            |op: &DhtOp| {
                if let Some(entry) = op.entry() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    facts![prism(
                        "action's entry hash",
                        |op: &mut DhtOp| op.action_entry_data_mut().map(|(hash, _)| hash),
                        eq("hash of matching entry", EntryHash::with_data_sync(entry)),
                    )]
                } else {
                    facts![always()]
                }
            }
        ),
        mapped("The Signature matches the Action", move |op: &DhtOp| {
            use holochain_keystore::AgentPubKeyExt;
            let action = op.action();
            let agent = action.author();
            let actual = tokio_helper::block_forever_on(agent.sign(&keystore, &action))
                .expect("Can sign the action");
            facts![lens("signature", DhtOp::signature_mut, eq_(actual))]
        })
    ]
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use holochain_keystore::test_keystore::spawn_test_keystore;

    use super::*;
    use holochain_zome_types::action::facts as action_facts;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_valid_dht_op() {
        // TODO: Must add constraint on dht op variant wrt action variant

        let mut uu = Unstructured::new(&NOISE);
        let u = &mut uu;
        let keystore = spawn_test_keystore().await.unwrap();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();

        let e = Entry::arbitrary(u).unwrap();

        let mut hn = not_(action_facts::is_new_entry_action()).build(u);
        *hn.author_mut() = agent.clone();

        let mut he = action_facts::is_new_entry_action().build(u);
        *he.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        let mut he = Action::from(he);
        *he.author_mut() = agent.clone();

        let se = agent.sign(&keystore, &he).await.unwrap();
        let sn = agent.sign(&keystore, &hn).await.unwrap();

        let op1 = DhtOp::StoreCommit(se.clone(), he.clone(), Some(Box::new(e.clone())));
        let op2 = DhtOp::StoreCommit(se.clone(), he.clone(), None);
        let op3 = DhtOp::StoreCommit(sn.clone(), hn.clone(), Some(Box::new(e.clone())));
        let op4 = DhtOp::StoreCommit(sn.clone(), hn.clone(), None);

        let fact = valid_dht_op(keystore);

        fact.check(&op1).unwrap();
        assert!(fact.check(&op2).is_err());
        assert!(fact.check(&op3).is_err());
        fact.check(&op4).unwrap();
    }
}

impl DhtOp {
    /// Mutable access to the Signature
    pub fn signature_mut(&mut self) -> &mut Signature {
        match self {
            DhtOp::StoreCommit(s, _, _) => s,
            DhtOp::StoreEntry(s, _, _) => s,
            DhtOp::RegisterAgentActivity(s, _) => s,
            DhtOp::RegisterUpdatedContent(s, _, _) => s,
            DhtOp::RegisterUpdatedCommit(s, _, _) => s,
            DhtOp::RegisterDeletedBy(s, _) => s,
            DhtOp::RegisterDeletedEntryAction(s, _) => s,
            DhtOp::RegisterAddLink(s, _) => s,
            DhtOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Mutable access to the seq of the Action, if applicable
    pub fn action_seq_mut(&mut self) -> Option<&mut u32> {
        match self {
            DhtOp::StoreCommit(_, ref mut h, _) => h.action_seq_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.action_seq_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.action_seq_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => Some(&mut h.action_seq),
            DhtOp::RegisterUpdatedCommit(_, ref mut h, _) => Some(&mut h.action_seq),
            DhtOp::RegisterDeletedBy(_, ref mut h) => Some(&mut h.action_seq),
            DhtOp::RegisterDeletedEntryAction(_, ref mut h) => Some(&mut h.action_seq),
            DhtOp::RegisterAddLink(_, ref mut h) => Some(&mut h.action_seq),
            DhtOp::RegisterRemoveLink(_, ref mut h) => Some(&mut h.action_seq),
        }
    }

    /// Mutable access to the entry data of the Action, if applicable
    pub fn action_entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            DhtOp::StoreCommit(_, ref mut h, _) => h.entry_data_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.entry_data_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.entry_data_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            DhtOp::RegisterUpdatedCommit(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            _ => None,
        }
    }
}
