#![allow(clippy::await_holding_lock)]

use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};

use holo_hash::{ActionHash, AgentPubKey};
use holochain_types::{inline_zome::InlineZomeSet, prelude::*};

use crate::{core::ribosome::guest_callback::validate::ValidateResult, sweettest::*};

const ZOME_A_0: &str = "ZOME_A_0";
const ZOME_A_1: &str = "ZOME_A_1";
const ZOME_B_0: &str = "ZOME_B_0";
const ZOME_B_1: &str = "ZOME_B_1";

const ALICE: &str = "ALICE";
const BOB: &str = "BOB";

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct Event {
    action: ActionLocation,
    op_type: ChainOpType,
    called_zome: &'static str,
    with_zome_index: Option<ZomeIndex>,
    with_entry_def_index: Option<EntryDefIndex>,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            action: Default::default(),
            op_type: ChainOpType::RegisterAgentActivity,
            called_zome: Default::default(),
            with_zome_index: Default::default(),
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
        event.op_type = ChainOpType::RegisterAgentActivity;
        self.all_zomes(event.clone());
        event.op_type = ChainOpType::StoreRecord;
        self.all_zomes(event.clone());
    }

    fn activity_all_zomes(&mut self, mut event: Event) {
        event.op_type = ChainOpType::RegisterAgentActivity;
        self.all_zomes(event.clone());
    }

    fn zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
        for zome in zomes {
            event.called_zome = *zome;
            self.0.insert(event.clone());
        }
    }

    fn activity_and_record_for_zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
        event.op_type = ChainOpType::RegisterAgentActivity;

        self.zomes(event.clone(), zomes);

        event.op_type = ChainOpType::StoreRecord;

        self.zomes(event.clone(), zomes);
    }

    fn record_for_zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
        event.op_type = ChainOpType::StoreRecord;

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

        event.op_type = ChainOpType::StoreEntry;
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
    holochain_trace::test_run();
    let entry_def_a = EntryDef::default_from_id("a");
    let entry_def_b = EntryDef::default_from_id("b");
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

    type AgentsMutex = Arc<parking_lot::Mutex<HashMap<AgentPubKey, &'static str>>>;

    let agents: AgentsMutex = Arc::new(parking_lot::Mutex::new(HashMap::new()));

    let validation_callback =
        |zome: &'static str, agents: AgentsMutex, events: tokio::sync::mpsc::Sender<Event>| {
            move |_api: BoxApi, op: Op| {
                let agents = agents.lock();
                let event = match op {
                    Op::StoreRecord(StoreRecord { record }) => Event {
                        action: ActionLocation::new(record.action().clone(), &agents),
                        op_type: ChainOpType::StoreRecord,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                    Op::StoreEntry(StoreEntry { action, .. }) => {
                        let (with_entry_def_index, with_zome_index) =
                            match action.hashed.content.app_entry_def().cloned() {
                                Some(AppEntryDef {
                                    entry_index,
                                    zome_index,
                                    ..
                                }) => (Some(entry_index), Some(zome_index)),
                                _ => (None, None),
                            };
                        Event {
                            action: ActionLocation::new(action.hashed.content.clone(), &agents),
                            op_type: ChainOpType::StoreEntry,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterUpdate(RegisterUpdate { update, .. }) => {
                        let (with_entry_def_index, with_zome_index) = match update.hashed.entry_type
                        {
                            EntryType::App(AppEntryDef {
                                entry_index,
                                zome_index,
                                ..
                            }) => (Some(entry_index), Some(zome_index)),
                            _ => (None, None),
                        };
                        Event {
                            action: ActionLocation::new(update.hashed.content.clone(), &agents),
                            op_type: ChainOpType::RegisterUpdatedContent,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterDelete(RegisterDelete { delete, .. }) => {
                        let (with_entry_def_index, with_zome_index) =
                            match (*delete.hashed).clone().into_action().entry_type() {
                                Some(EntryType::App(AppEntryDef {
                                    entry_index,
                                    zome_index,
                                    ..
                                })) => (Some(*entry_index), Some(*zome_index)),
                                _ => (None, None),
                            };
                        Event {
                            action: ActionLocation::new(delete.hashed.content.clone(), &agents),
                            op_type: ChainOpType::RegisterDeletedBy,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => Event {
                        action: ActionLocation::new(action.action().clone(), &agents),
                        op_type: ChainOpType::RegisterAgentActivity,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                    Op::RegisterCreateLink(RegisterCreateLink { create_link, .. }) => Event {
                        action: ActionLocation::new(create_link.hashed.content.clone(), &agents),
                        op_type: ChainOpType::RegisterAddLink,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                    Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => Event {
                        action: ActionLocation::new(delete_link.hashed.content.clone(), &agents),
                        op_type: ChainOpType::RegisterRemoveLink,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                };
                events.try_send(event).unwrap();
                Ok(ValidateResult::Valid)
            }
        };

    let mut conductors =
        SweetConductorBatch::from_config(2, SweetConductorConfig::standard()).await;

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
        [
            ("zome1".into(), "integrity_zome1".into()),
            ("zome2".into(), "integrity_zome2".into()),
        ],
    )
    .with_dependency("zome1", "integrity_zome1")
    .with_dependency("zome2", "integrity_zome2")
    .function("zome1", "create_a", call_back_a("integrity_zome1"))
    .function("zome1", "create_b", call_back_b("integrity_zome1"))
    .function(
        "integrity_zome1",
        "validate",
        validation_callback(ZOME_A_0, agents.clone(), events_tx.clone()),
    )
    .function("zome2", "create_a", call_back_a("integrity_zome2"))
    .function("zome2", "create_b", call_back_b("integrity_zome2"))
    .function(
        "integrity_zome2",
        "validate",
        validation_callback(ZOME_A_1, agents.clone(), events_tx.clone()),
    );
    let (dna_file_a, _, _) = SweetDnaFile::from_inline_zomes("".into(), zomes).await;

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
        [
            ("zome1".into(), "integrity_zome1".into()),
            ("zome2".into(), "integrity_zome2".into()),
        ],
    )
    .with_dependency("zome1", "integrity_zome1")
    .with_dependency("zome2", "integrity_zome2")
    .function("zome1", "create_a", call_back_a("integrity_zome1"))
    .function("zome1", "create_b", call_back_b("integrity_zome2"))
    .function(
        "integrity_zome1",
        "validate",
        validation_callback(ZOME_B_0, agents.clone(), events_tx.clone()),
    )
    .function("zome2", "create_a", call_back_a("integrity_zome2"))
    .function("zome2", "create_b", call_back_b("integrity_zome2"))
    .function(
        "integrity_zome2",
        "validate",
        validation_callback(ZOME_B_1, agents.clone(), events_tx.clone()),
    );

    let (dna_file_b, _, _) = SweetDnaFile::from_inline_zomes("".into(), zomes).await;

    let (alice, bob) = {
        let mut agents = agents.lock();

        let app = conductors[0]
            .setup_app("test_app", &[dna_file_a.clone()])
            .await
            .unwrap();
        let (alice,) = app.into_tuple();
        let app = conductors[1]
            .setup_app("test_app", &[dna_file_b.clone()])
            .await
            .unwrap();
        let (bob,) = app.into_tuple();

        agents.insert(alice.agent_pubkey().clone(), ALICE);
        agents.insert(bob.agent_pubkey().clone(), BOB);

        (alice, bob)
    };

    conductors.exchange_peer_info().await;

    let _: ActionHash = conductors[0]
        .call(&alice.zome("zome1"), "create_a", ())
        .await;

    await_consistency(10, [&alice, &bob]).await.unwrap();

    let mut expected = Expected(HashSet::new());

    expected.genesis(ALICE, &[ZOME_B_0, ZOME_B_1]);
    expected.genesis(BOB, &[ZOME_A_0, ZOME_A_1]);

    expected.init(ALICE);

    let mut event = Event {
        action: ActionLocation::expected(ALICE, ActionType::Create, 4),
        ..Default::default()
    };
    expected.activity_all_zomes(event.clone());
    expected.record_for_zomes(event.clone(), &[ZOME_A_0, ZOME_B_0]);

    event.op_type = ChainOpType::StoreEntry;
    event.called_zome = ZOME_A_0;
    event.with_zome_index = Some(ZomeIndex(0));
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
            let events = events.into_iter().fold(String::new(), |mut acc, s| {
                acc.push_str(&s);
                acc.push('\n');
                acc
            });
            writeln!(&mut s, "{}", events).unwrap();

            panic!("{}", s);
        }
    }
    if !expected.0.is_empty() {
        let mut events: Vec<String> = expected.0.iter().map(Event::to_string).collect();
        events.sort();
        let events = events.into_iter().fold(String::new(), |mut acc, s| {
            acc.push_str(&s);
            acc.push('\n');
            acc
        });

        panic!(
            "The following ops were expected to be validated but never were: \n{}",
            events
        );
    }
}
