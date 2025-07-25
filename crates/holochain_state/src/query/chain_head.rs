use super::*;
use crate::prelude::HeadInfo;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct AuthoredChainHeadQuery;

impl AuthoredChainHeadQuery {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthoredChainHeadQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl Query for AuthoredChainHeadQuery {
    type Item = Judged<SignedActionHashed>;
    type State = Option<SignedActionHashed>;
    type Output = Option<HeadInfo>;

    fn query(&self) -> String {
        "
        SELECT Action.blob, Action.hash
        FROM Action
        ORDER BY Action.seq DESC LIMIT 1
        "
        .into()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(None)
    }

    fn fold(&self, state: Self::State, sh: Self::Item) -> StateQueryResult<Self::State> {
        // We don't need the validation status from this point.
        let sh = sh.data;
        // Simple maximum finding
        Ok(Some(match state {
            None => sh,
            Some(old) => {
                if sh.action().action_seq() > old.action().action_seq() {
                    sh
                } else {
                    old
                }
            }
        }))
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state.map(|sh| {
            let seq = sh.action().action_seq();
            let timestamp = sh.action().timestamp();
            let action = sh.hashed.hash;
            HeadInfo {
                action,
                seq,
                timestamp,
            }
        }))
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_and_hash_to_action("blob", "hash");
        // Valid because the data is authored.
        Arc::new(move |r| Ok(Judged::valid(f(r)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutations::{insert_action, insert_op_lite};
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holo_hash::fixt::DhtOpHashFixturator;
    use holochain_sqlite::rusqlite::Connection;
    use holochain_sqlite::rusqlite::TransactionBehavior;
    use holochain_sqlite::schema::SCHEMA_CELL;
    use holochain_types::dht_op::DhtOpLite;
    use holochain_types::dht_op::OpOrder;

    #[test]
    fn test_chain_head_query() {
        holochain_trace::test_run();
        let mut conn = Connection::open_in_memory().unwrap();
        SCHEMA_CELL.initialize(&mut conn, None).unwrap();

        let mut txn = conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .unwrap();

        let author = fixt!(AgentPubKey);

        // Create 5 consecutive actions for the authoring agent.
        // There can not be any actions by different authors in the chain.
        let mut actions: Vec<_> = vec![
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
        ]
        .into_iter()
        .enumerate()
        .map(|(seq, action)| {
            let mut chain_action = action.clone();
            chain_action.action_seq = seq as u32;
            chain_action.author = author.clone();
            SignedActionHashed::with_presigned(
                // A chain made entirely of InitZomesComplete actions is totally invalid,
                // but we don't need a valid chain for this test,
                // we just need an ordered sequence of actions
                ActionHashed::from_content_sync(InitZomesComplete::from_builder(chain_action)),
                fixt!(Signature),
            )
        })
        .collect();

        // The 5th action should be the head for our author's chain.
        let expected_head = actions[4].clone();
        // Shuffle so the head will sometimes be in scratch and sometimes be in the database and not always the last action by our author.
        actions.shuffle(&mut rand::rng());

        for action in &actions[..2] {
            let hash = action.action_address();
            let op = DhtOpLite::from(ChainOpLite::StoreRecord(
                hash.clone(),
                None,
                hash.clone().into(),
            ));
            let op_order = OpOrder::new(op.get_type(), action.action().timestamp());
            insert_action(&mut txn, action).unwrap();
            insert_op_lite(
                &mut txn,
                &op,
                &fixt!(DhtOpHash),
                &op_order,
                &action.action().timestamp(),
                0,
                None,
            )
            .unwrap();
        }

        let mut scratch = Scratch::new();
        for action in &actions[2..] {
            scratch.add_action(action.clone(), ChainTopOrdering::default());
        }

        let query = AuthoredChainHeadQuery::new();

        let head = query.run(DbScratch::new(&[&mut txn], &scratch)).unwrap();
        assert_eq!(
            head.unwrap(),
            HeadInfo {
                action: expected_head.as_hash().clone(),
                seq: expected_head.action().action_seq(),
                timestamp: expected_head.action().timestamp()
            }
        );
    }
}
