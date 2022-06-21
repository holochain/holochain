use ::fixt::prelude::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_types::activity::ChainItems;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;

use holochain_zome_types::*;

#[derive(Debug)]
pub struct ActivityTestData {
    pub hash_ops: Vec<DhtOpHashed>,
    pub noise_ops: Vec<DhtOpHashed>,
    pub store_ops: Vec<DhtOpHashed>,
    pub agent: AgentPubKey,
    pub query_filter: ChainQueryFilter,
    pub valid_hashes: ChainItems<ActionHash>,
    pub valid_commits: ChainItems<Commit>,
    pub chain_head: ChainHead,
    pub highest_observed: HighestObserved,
}

impl ActivityTestData {
    pub fn valid_chain_scenario() -> Self {
        // The agent we are querying.
        let agent = fixt!(AgentPubKey);

        // An entry that all actions can use to make things simpler.
        let entry = Entry::App(fixt!(AppEntryBytes));
        let entry_hash = EntryHash::with_data_sync(&entry);

        let to_op =
            |h| DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(fixt!(Signature), h));

        let to_commit_and_op = |h: Action| {
            let sig = fixt!(Signature);
            // let e = Entry::App(fixt!(AppEntryBytes));
            let op = DhtOpHashed::from_content_sync(DhtOp::StoreCommit(
                sig.clone(),
                h.clone(),
                Some(Box::new(entry.clone())),
            ));
            let shh = SignedActionHashed::with_presigned(ActionHashed::from_content_sync(h), sig);
            (Commit::new(shh, Some(entry.clone())), op)
        };

        let to_commit_dna_op = |h: Action| {
            let sig = fixt!(Signature);
            let op =
                DhtOpHashed::from_content_sync(DhtOp::StoreCommit(sig.clone(), h.clone(), None));
            let shh = SignedActionHashed::with_presigned(ActionHashed::from_content_sync(h), sig);
            (Commit::new(shh, None), op)
        };

        // The hashes we are expecting to get returned by the below activity set.
        let mut valid_hashes = Vec::new();

        // The commits on the chain. Needs to match the activity set.
        let mut valid_commits = Vec::new();

        // The store commit ops for the actual data on the chain which should
        // match the set of activity ops.
        let mut store_ops = Vec::new();

        // A set of activity ops:
        // - Must be on the above agents chain.
        // - Create a valid, unbroken chain.
        // - All actions are valid:
        //    - Prev hash actually match prev action's hash
        //    - Seq numbers are in order.
        //    - First action must be a Dna.
        let mut hash_ops = Vec::new();
        let mut dna = fixt!(Dna);
        dna.author = agent.clone();
        let dna = Action::Dna(dna);

        // Insert the dna
        let (el, op) = to_commit_dna_op(dna.clone());
        valid_commits.push(el);
        store_ops.push(op);
        hash_ops.push(to_op(dna.clone()));

        let creates: Vec<_> = CreateFixturator::new(Unpredictable)
            .enumerate()
            .take(50)
            .collect();
        let mut prev_hash = ActionHash::with_data_sync(&dna);
        valid_hashes.push((0, prev_hash.clone()));
        for (seq, mut create) in creates {
            let action_seq = (seq + 1) as u32;
            create.author = agent.clone();
            create.action_seq = action_seq;
            create.prev_action = prev_hash.clone();
            create.entry_hash = entry_hash.clone();
            let action = Action::Create(create);
            prev_hash = ActionHash::with_data_sync(&action);
            hash_ops.push(to_op(action.clone()));

            valid_hashes.push((action_seq, prev_hash.clone()));

            let (el, op) = to_commit_and_op(action);
            valid_commits.push(el);
            store_ops.push(op);
        }

        // The head of the chain is the last valid hash
        // because we are going to insert all ops as valid and integrated.
        let last = valid_hashes.last().unwrap();
        let chain_head = ChainHead {
            action_seq: last.0,
            hash: last.1.clone(),
        };

        // Highest Observed is the same as the chain head.
        let highest_observed = HighestObserved {
            action_seq: last.0,
            hash: vec![last.1.clone()],
        };

        // We just want a simple query filter to get back the full chain.
        let query_filter = QueryFilter::new();

        // Finally add some random noise so we know we are getting the correct items.
        let noise_ops = ActionFixturator::new(Unpredictable)
            .take(50)
            .map(to_op)
            .collect();

        Self {
            hash_ops,
            agent,
            query_filter,
            valid_hashes: ChainItems::Hashes(valid_hashes),
            highest_observed,
            chain_head,
            noise_ops,
            store_ops,
            valid_commits: ChainItems::Full(valid_commits),
        }
    }
}
