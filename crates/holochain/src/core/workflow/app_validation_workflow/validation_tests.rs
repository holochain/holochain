use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

use holo_hash::{ActionHash, AgentPubKey};
use holochain_types::{dht_op::DhtOpType, inline_zome::InlineZomeSet};
use holochain_zome_types::{
    Action, ActionType, AppEntryType, BoxApi, ChainTopOrdering, CreateInput, Entry, EntryDef,
    EntryDefIndex, EntryVisibility, Op, TryInto, ZomeId,
};

use crate::{
    core::ribosome::guest_callback::validate::ValidateResult, sweettest::*,
    test_utils::consistency_10s,
};

const ZOME_A_0: &'static str = "ZOME_A_0";
const ZOME_A_1: &'static str = "ZOME_A_1";
const ZOME_B_0: &'static str = "ZOME_B_0";
const ZOME_B_1: &'static str = "ZOME_B_1";

const ALICE: &'static str = "ALICE";
const BOB: &'static str = "BOB";

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct Event {
    action: ActionLocation,
    op_type: DhtOpType,
    called_zome: &'static str,
    with_zome_id: Option<ZomeId>,
    with_entry_def_index: Option<EntryDefIndex>,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            action: Default::default(),
            op_type: DhtOpType::RegisterAgentActivity,
            called_zome: Default::default(),
            with_zome_id: Default::default(),
            with_entry_def_index: Default::default(),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Default)]
struct ActionLocation {
    agent: &'static str,
    action_type: String,
    seq: u32,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.with_entry_def_index {
            Some(e) => write!(
                f,
                "{}:{}:{}:entry_id({})",
                self.called_zome, self.op_type, self.action, e.0
            ),
            None => write!(f, "{}:{}:{}", self.called_zome, self.op_type, self.action),
        }
    }
}

impl std::fmt::Display for ActionLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.agent, self.action_type, self.seq)
    }
}

impl ActionLocation {
    fn new(action: impl Into<Action>, agents: &HashMap<AgentPubKey, &'static str>) -> Self {
        let action = action.into();
        Self {
            agent: agents.get(action.author()).unwrap(),
            action_type: action.action_type().to_string(),
            seq: action.action_seq(),
        }
    }

    fn expected(agent: &'static str, action_type: ActionType, seq: u32) -> Self {
        Self {
            agent,
            action_type: action_type.to_string(),
            seq,
        }
    }
}
struct Expected(HashSet<Event>);

impl Expected {
    fn all_zomes(&mut self, mut event: Event) {
        event.called_zome = ZOME_A_0;
        self.0.insert(event.clone());
        event.called_zome = ZOME_A_1;
        self.0.insert(event.clone());
        event.called_zome = ZOME_B_0;
        self.0.insert(event.clone());
        event.called_zome = ZOME_B_1;
        self.0.insert(event);
    }

    fn activity_and_record_all_zomes(&mut self, mut event: Event) {
        event.op_type = DhtOpType::RegisterAgentActivity;
        self.all_zomes(event.clone());
        event.op_type = DhtOpType::StoreRecord;
        self.all_zomes(event.clone());
    }

    fn zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
        for zome in zomes {
            event.called_zome = *zome;
            self.0.insert(event.clone());
        }
    }

    fn activity_and_record_for_zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
        event.op_type = DhtOpType::RegisterAgentActivity;

        self.zomes(event.clone(), zomes);

        event.op_type = DhtOpType::StoreRecord;

        self.zomes(event.clone(), zomes);
    }

    fn genesis(&mut self, agent: &'static str, zomes: &[&'static str]) {
        let event = Event {
            action: ActionLocation::expected(agent, ActionType::Dna, 0),
            ..Default::default()
        };
        self.activity_and_record_for_zomes(event.clone(), zomes);

        let event = Event {
            action: ActionLocation::expected(agent, ActionType::AgentValidationPkg, 1),
            ..Default::default()
        };
        self.activity_and_record_for_zomes(event.clone(), zomes);

        let mut event = Event {
            action: ActionLocation::expected(agent, ActionType::Create, 2),
            ..Default::default()
        };
        self.activity_and_record_for_zomes(event.clone(), zomes);

        event.op_type = DhtOpType::StoreEntry;
        self.zomes(event.clone(), zomes);
    }

    fn init(&mut self, agent: &'static str) {
        let event = Event {
            action: ActionLocation::expected(agent, ActionType::InitZomesComplete, 3),
            ..Default::default()
        };
        self.activity_and_record_all_zomes(event.clone());
    }
}

#[tokio::test(flavor = "multi_thread")]
/// Test that all ops are created and the correct zomes
/// are called for each op.
async fn app_validation_ops() {
    observability::test_run().ok();
    let entry_def_a = EntryDef::default_with_id("a");
    let entry_def_b = EntryDef::default_with_id("b");
    let call_back_a = |_zome_name: &'static str| {
        move |api: BoxApi, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        }
    };
    let call_back_b = |_zome_name: &'static str| {
        move |api: BoxApi, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        }
    };

    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(100);

    let validation_callback =
        |zome: &'static str,
         agents: HashMap<AgentPubKey, &'static str>,
         events: tokio::sync::mpsc::Sender<Event>| {
            move |_api: BoxApi, op: Op| {
                let event = match op {
                    Op::StoreRecord { record } => Event {
                        action: ActionLocation::new(record.action().clone(), &agents),
                        op_type: DhtOpType::StoreRecord,
                        called_zome: zome,
                        with_zome_id: None,
                        with_entry_def_index: None,
                    },
                    Op::StoreEntry { action, .. } => {
                        let (with_entry_def_index, with_zome_id) =
                            match action.hashed.content.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(id), Some(zome_id)),
                                _ => (None, None),
                            };
                        Event {
                            action: ActionLocation::new(action.hashed.content.clone(), &agents),
                            op_type: DhtOpType::StoreEntry,
                            called_zome: zome,
                            with_zome_id,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterUpdate {
                        update,
                        original_action,
                        ..
                    } => {
                        let (with_entry_def_index, with_zome_id) =
                            match original_action.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(id), Some(zome_id)),
                                _ => (None, None),
                            };
                        Event {
                            action: ActionLocation::new(update.hashed.content.clone(), &agents),
                            op_type: DhtOpType::RegisterUpdatedContent,
                            called_zome: zome,
                            with_zome_id,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterDelete {
                        delete,
                        original_action,
                        ..
                    } => {
                        let (with_entry_def_index, with_zome_id) =
                            match original_action.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(id), Some(zome_id)),
                                _ => (None, None),
                            };
                        Event {
                            action: ActionLocation::new(delete.hashed.content.clone(), &agents),
                            op_type: DhtOpType::RegisterDeletedBy,
                            called_zome: zome,
                            with_zome_id,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterAgentActivity { action } => Event {
                        action: ActionLocation::new(action.action().clone(), &agents),
                        op_type: DhtOpType::RegisterAgentActivity,
                        called_zome: zome,
                        with_zome_id: None,
                        with_entry_def_index: None,
                    },
                    Op::RegisterCreateLink { create_link, .. } => Event {
                        action: ActionLocation::new(create_link.hashed.content.clone(), &agents),
                        op_type: DhtOpType::RegisterAddLink,
                        called_zome: zome,
                        with_zome_id: None,
                        with_entry_def_index: None,
                    },
                    Op::RegisterDeleteLink { delete_link, .. } => Event {
                        action: ActionLocation::new(delete_link.hashed.content.clone(), &agents),
                        op_type: DhtOpType::RegisterRemoveLink,
                        called_zome: zome,
                        with_zome_id: None,
                        with_entry_def_index: None,
                    },
                };
                events.try_send(event).unwrap();
                Ok(ValidateResult::Valid)
            }
        };

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let alice = SweetAgents::one(conductors[0].keystore()).await;
    let bob = SweetAgents::one(conductors[1].keystore()).await;

    let mut agents = HashMap::new();
    agents.insert(alice.clone(), ALICE);
    agents.insert(bob.clone(), BOB);

    let zomes = InlineZomeSet::new(
        [
            (
                "integrity_zome1",
                "integrity_a".to_string(),
                vec![entry_def_a.clone(), entry_def_b.clone()],
                0,
            ),
            (
                "integrity_zome2",
                "integrity_b".to_string(),
                vec![entry_def_a.clone(), entry_def_b.clone()],
                0,
            ),
        ],
        [("zome1", "a".to_string()), ("zome2", "b".to_string())],
    )
    .with_dependency("zome1", "integrity_zome1")
    .with_dependency("zome2", "integrity_zome2")
    .callback("zome1", "create_a", call_back_a("integrity_zome1"))
    .callback("zome1", "create_b", call_back_b("integrity_zome1"))
    .callback(
        "integrity_zome1",
        "validate",
        validation_callback(ZOME_A_0, agents.clone(), events_tx.clone()),
    )
    .callback("zome2", "create_a", call_back_a("integrity_zome2"))
    .callback("zome2", "create_b", call_back_b("integrity_zome2"))
    .callback(
        "integrity_zome2",
        "validate",
        validation_callback(ZOME_A_1, agents.clone(), events_tx.clone()),
    );
    let (dna_file_a, _, _) = SweetDnaFile::from_inline_zomes("".into(), zomes)
        .await
        .unwrap();

    let zomes = InlineZomeSet::new(
        [
            (
                "integrity_zome1",
                "integrity_a".to_string(),
                vec![entry_def_a.clone(), entry_def_b.clone()],
                0,
            ),
            (
                "integrity_zome2",
                "integrity_b".to_string(),
                vec![entry_def_a.clone(), entry_def_b.clone()],
                0,
            ),
        ],
        [("zome1", "a".to_string()), ("zome2", "b".to_string())],
    )
    .with_dependency("zome1", "integrity_zome1")
    .with_dependency("zome2", "integrity_zome2")
    .callback("zome1", "create_a", call_back_a("integrity_zome1"))
    .callback("zome1", "create_b", call_back_b("integrity_zome2"))
    .callback(
        "integrity_zome1",
        "validate",
        validation_callback(ZOME_B_0, agents.clone(), events_tx.clone()),
    )
    .callback("zome2", "create_a", call_back_a("integrity_zome2"))
    .callback("zome2", "create_b", call_back_b("integrity_zome2"))
    .callback(
        "integrity_zome2",
        "validate",
        validation_callback(ZOME_B_1, agents.clone(), events_tx.clone()),
    );

    let (dna_file_b, _, _) = SweetDnaFile::from_inline_zomes("".into(), zomes)
        .await
        .unwrap();
    let app = conductors[0]
        .setup_app_for_agent(&"test_app", alice.clone(), &[dna_file_a.clone()])
        .await
        .unwrap();
    let (alice,) = app.into_tuple();
    let app = conductors[1]
        .setup_app_for_agent(&"test_app", bob.clone(), &[dna_file_b.clone()])
        .await
        .unwrap();
    let (bob,) = app.into_tuple();
    conductors.exchange_peer_info().await;

    let _: ActionHash = conductors[0]
        .call(&alice.zome("zome1"), "create_a", ())
        .await;

    consistency_10s(&[&alice, &bob]).await;

    let mut expected = Expected(HashSet::new());

    expected.genesis(ALICE, &[ZOME_B_0, ZOME_B_1]);
    expected.genesis(BOB, &[ZOME_A_0, ZOME_A_1]);

    expected.init(ALICE);

    let mut event = Event {
        action: ActionLocation::expected(ALICE, ActionType::Create, 4),
        ..Default::default()
    };
    expected.activity_and_record_all_zomes(event.clone());

    event.op_type = DhtOpType::StoreEntry;
    event.called_zome = ZOME_A_0;
    event.with_zome_id = Some(ZomeId(0));
    event.with_entry_def_index = Some(0.into());
    expected.0.insert(event.clone());

    event.called_zome = ZOME_B_0;
    expected.0.insert(event.clone());

    let mut received = HashSet::new();

    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_secs(5), events_rx.recv()).await
    {
        if !received.insert(event.clone()) {
            panic!("Got {} twice", event);
        }
        if !expected.0.remove(&event) {
            let mut s = String::new();
            writeln!(&mut s, "Got event {} that was missing from:", event).unwrap();
            let mut events: Vec<String> = expected.0.iter().map(Event::to_string).collect();
            events.sort();
            let events: String = events.into_iter().map(|s| format!("{}\n", s)).collect();
            writeln!(&mut s, "{}", events).unwrap();

            panic!("{}", s);
        }
    }
    if !expected.0.is_empty() {
        let mut events: Vec<String> = expected.0.iter().map(Event::to_string).collect();
        events.sort();
        let events: String = events.into_iter().map(|s| format!("{}\n", s)).collect();

        panic!(
            "The following ops were expected to be validated but never were: \n{}",
            events
        );
    }
}
