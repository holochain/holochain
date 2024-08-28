use ::fixt::prelude::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_types::activity::ChainItems;
use holochain_types::dht_op::ChainOp;
use holochain_types::dht_op::ChainOpHashed;
use holochain_types::prelude::NewEntryAction;
use holochain_zome_types::prelude::*;

/// A collection of fixtures used to create scenarios for testing the Cascade
#[derive(Debug, Clone)]
pub struct ActivityTestData {
    /// AgentActivity ops to expect being able to get
    pub agent_activity_ops: Vec<ChainOpHashed>,
    /// "Noise", to ensure that the query filter is doing its job
    pub noise_agent_activity_ops: Vec<ChainOpHashed>,
    /// StoreRecord ops to expect being able to get
    pub store_entry_ops: Vec<ChainOpHashed>,
    /// The author of the chain
    pub agent: AgentPubKey,
    /// The expected hash return values
    pub valid_hashes: ChainItems,
    /// The expected action return values
    pub valid_actions: ChainItems,
    /// The expected record return values
    pub valid_records: ChainItems,
    /// The head of the chain produced
    pub chain_head: ChainHead,
    /// Same as the chain_head
    pub highest_observed: HighestObserved,
}

impl ActivityTestData {
    /// Construct a set of test fixtures representing a valid source chain
    pub fn valid_chain_scenario() -> Self {
        // The agent we are querying.
        let agent = fixt!(AgentPubKey);

        // An entry that all actions can use to make things simpler.
        let entry = Entry::App(fixt!(AppEntryBytes));
        let entry_hash = EntryHash::with_data_sync(&entry);

        let to_agent_activity_op =
            |h, sig| ChainOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(sig, h));

        let to_record_and_op = |a: Action, sig: Signature| {
            let op = ChainOpHashed::from_content_sync(ChainOp::StoreEntry(
                sig.clone(),
                match a {
                    Action::Create(ref c) => NewEntryAction::Create(c.clone()),
                    _ => unreachable!(),
                },
                entry.clone().into(),
            ));
            let shh = SignedActionHashed::with_presigned(ActionHashed::from_content_sync(a), sig);
            (Record::new(shh, Some(entry.clone())), op)
        };

        let to_record_dna_op = |a: Action, sig: Signature| {
            let op = ChainOpHashed::from_content_sync(ChainOp::StoreRecord(
                sig.clone(),
                a.clone(),
                RecordEntry::NA,
            ));
            let shh = SignedActionHashed::with_presigned(ActionHashed::from_content_sync(a), sig);
            (Record::new(shh, None), op)
        };

        // The hashes we are expecting to get returned by the below activity set.
        let mut valid_hashes = Vec::new();

        // The records on the chain. Needs to match the activity set.
        let mut valid_records = Vec::new();

        // The store record ops for the actual data on the chain which should
        // match the set of activity ops.
        let mut store_entry_ops = Vec::new();

        // A set of activity ops:
        // - Must be on the above agents chain.
        // - Create a valid, unbroken chain.
        // - All actions are valid:
        //    - Prev hash actually match prev action's hash
        //    - Seq numbers are in order.
        //    - First action must be a Dna.
        let mut agent_activity_ops = Vec::new();
        let mut dna = fixt!(Dna);
        dna.author = agent.clone();
        let dna = Action::Dna(dna);

        // Insert the dna
        let dna_sig = fixt!(Signature);
        let (el, op) = to_record_dna_op(dna.clone(), dna_sig.clone());
        valid_records.push(el);
        store_entry_ops.push(op);
        agent_activity_ops.push(to_agent_activity_op(dna.clone(), dna_sig));

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
            create.entry_type = EntryType::App(AppEntryDef::new(
                1.into(),
                1.into(),
                EntryVisibility::Public,
            ));
            let action = Action::Create(create);
            let sig = fixt!(Signature);
            prev_hash = ActionHash::with_data_sync(&action);
            agent_activity_ops.push(to_agent_activity_op(action.clone(), sig.clone()));

            valid_hashes.push((action_seq, prev_hash.clone()));

            let (el, op) = to_record_and_op(action, sig);
            valid_records.push(el);
            store_entry_ops.push(op);
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

        // Finally add some random noise, so we know we are getting the correct items.
        let noise_agent_activity_ops = ActionFixturator::new(Unpredictable)
            .take(50)
            .map(|a| to_agent_activity_op(a, fixt!(Signature)))
            .collect();

        Self {
            agent_activity_ops,
            noise_agent_activity_ops,
            store_entry_ops,
            agent,
            valid_hashes: ChainItems::Hashes(valid_hashes),
            valid_actions: ChainItems::FullActions(
                valid_records
                    .iter()
                    .map(|r| r.signed_action.clone())
                    .collect(),
            ),
            valid_records: ChainItems::FullRecords(valid_records),
            highest_observed,
            chain_head,
        }
    }
}
