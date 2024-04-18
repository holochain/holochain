use holo_hash::*;
use holochain_sqlite::rusqlite::*;
use holochain_zome_types::prelude::*;
use std::fmt::Debug;

use crate::prelude::HeadInfo;

use super::Params;
use super::*;

#[derive(Debug, Clone)]
pub struct ChainHeadQuery(Arc<AgentPubKey>);

impl ChainHeadQuery {
    pub fn new(agent: Arc<AgentPubKey>) -> Self {
        Self(agent)
    }
}

impl Query for ChainHeadQuery {
    type Item = Judged<SignedActionHashed>;
    type State = Option<SignedActionHashed>;
    type Output = Option<HeadInfo>;

    fn query(&self) -> String {
        "
        SELECT Action.blob, Action.hash
        FROM Action
        JOIN DhtOp ON DhtOp.action_hash = Action.hash
        WHERE Action.author = :author AND Action.hash IS NOT NULL
        ORDER BY Action.seq DESC LIMIT 1
        "
        .into()
    }

    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":author": self.0,
        };
        params.to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(None)
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let author = self.0.clone();
        // NB: it's a little redundant to filter on author, since we should never
        // be putting any actions by other authors in our scratch, but it
        // certainly doesn't hurt to be consistent.
        let f = move |action: &SignedActionHashed| *action.action().author() == *author;
        Box::new(f)
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

        // Create 5 consecutive actions for the authoring agent,
        // as well as 5 other random actions, interspersed.
        let mut actions: Vec<_> = vec![
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
            fixt!(ActionBuilderCommon),
        ]
        .into_iter()
        .enumerate()
        .flat_map(|(seq, random_action)| {
            let mut chain_action = random_action.clone();
            chain_action.action_seq = seq as u32;
            chain_action.author = author.clone();
            vec![chain_action, random_action]
        })
        .map(|b| {
            SignedActionHashed::with_presigned(
                // A chain made entirely of InitZomesComplete actions is totally invalid,
                // but we don't need a valid chain for this test,
                // we just need an ordered sequence of actions
                ActionHashed::from_content_sync(InitZomesComplete::from_builder(b).into()),
                fixt!(Signature),
            )
        })
        .collect();

        // Other actions have a different author, so the 9th action should be the head for our author's chain
        let expected_head = actions[8].clone();
        // Shuffle so the head will sometimes be in scratch and sometimes be in the database and not always the last action by our author.
        actions.shuffle(&mut thread_rng());

        for action in &actions[..6] {
            let hash = action.action_address();
            let op = DhtOpLite::StoreRecord(hash.clone(), None, hash.clone().into());
            let op_order = OpOrder::new(op.get_type(), action.action().timestamp());
            insert_action(&mut txn, action).unwrap();
            insert_op_lite(
                &mut txn,
                &op,
                &fixt!(DhtOpHash),
                &op_order,
                &action.action().timestamp(),
            )
            .unwrap();
        }

        let mut scratch = Scratch::new();

        // It's also totally invalid for a call_zome scratch to contain actions
        // from other authors, but it doesn't matter here
        for action in &actions[6..] {
            scratch.add_action(action.clone(), ChainTopOrdering::default());
        }

        let query = ChainHeadQuery::new(Arc::new(author));

        let head = query.run(DbScratch::new(&[&mut txn], &scratch)).unwrap();
        // let head = query.run(Txn::from(&txn)).unwrap();
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
