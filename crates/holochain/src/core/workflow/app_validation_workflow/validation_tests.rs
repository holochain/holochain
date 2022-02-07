use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

use holo_hash::{AgentPubKey, HeaderHash};
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::{
    AppEntryType, BoxApi, ChainTopOrdering, CreateInput, Entry, EntryDef, EntryDefId,
    EntryDefIndex, Header, HeaderType, InlineZome, Op, TryInto, ZomeId,
};

use crate::{
    core::ribosome::guest_callback::validate::ValidateResult, sweettest::*,
    test_utils::consistency_10s,
};

#[tokio::test(flavor = "multi_thread")]
/// Test that all ops are created and the correct zomes
/// are called for each op.
async fn app_validation_ops() {
    observability::test_run().ok();
    let entry_def_a = EntryDef::default_with_id("a");
    let entry_def_b = EntryDef::default_with_id("b");
    let call_back_a = || {
        let entry_def_a = entry_def_a.clone();
        move |api: BoxApi, ()| {
            let entry_def_id: EntryDefId = entry_def_a.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                entry_def_id,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        }
    };
    let call_back_b = || {
        let entry_def_b = entry_def_b.clone();
        move |api: BoxApi, ()| {
            let entry_def_id: EntryDefId = entry_def_b.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                entry_def_id,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        }
    };

    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(100);
    const ZOME_A_0: &'static str = "ZOME_A_0";
    const ZOME_A_1: &'static str = "ZOME_A_1";
    const ZOME_B_0: &'static str = "ZOME_B_0";
    const ZOME_B_1: &'static str = "ZOME_B_1";

    const ALICE: &'static str = "ALICE";
    const BOB: &'static str = "BOB";

    #[derive(Debug, Hash, PartialEq, Eq, Clone)]
    struct Event {
        header: HeaderLocation,
        op_type: DhtOpType,
        called_zome: &'static str,
        with_zome_index: Option<ZomeId>,
        with_entry_def_index: Option<EntryDefIndex>,
    }

    impl Default for Event {
        fn default() -> Self {
            Self {
                header: Default::default(),
                op_type: DhtOpType::RegisterAgentActivity,
                called_zome: Default::default(),
                with_zome_index: Default::default(),
                with_entry_def_index: Default::default(),
            }
        }
    }

    #[derive(Debug, Hash, PartialEq, Eq, Clone, Default)]
    struct HeaderLocation {
        agent: &'static str,
        header_type: String,
        seq: u32,
    }

    impl std::fmt::Display for Event {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match (&self.with_zome_index, &self.with_entry_def_index) {
                (Some(z), None) => write!(
                    f,
                    "{}:{}:{}:zome_id({})",
                    self.called_zome, self.op_type, self.header, z
                ),
                (Some(z), Some(e)) => write!(
                    f,
                    "{}:{}:{}:zome_id({}):entry_id({})",
                    self.called_zome, self.op_type, self.header, z, e.0
                ),
                _ => write!(f, "{}:{}:{}", self.called_zome, self.op_type, self.header),
            }
        }
    }

    impl std::fmt::Display for HeaderLocation {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}:{}:{}", self.agent, self.header_type, self.seq)
        }
    }

    impl HeaderLocation {
        fn new(header: impl Into<Header>, agents: &HashMap<AgentPubKey, &'static str>) -> Self {
            let header = header.into();
            Self {
                agent: agents.get(header.author()).unwrap(),
                header_type: header.header_type().to_string(),
                seq: header.header_seq(),
            }
        }

        fn expect(agent: &'static str, header_type: HeaderType, seq: u32) -> Self {
            Self {
                agent,
                header_type: header_type.to_string(),
                seq,
            }
        }
    }

    let validation_callback =
        |zome: &'static str,
         agents: HashMap<AgentPubKey, &'static str>,
         events: tokio::sync::mpsc::Sender<Event>| {
            move |_api: BoxApi, op: Op| {
                let event = match op {
                    Op::StoreElement { element } => Event {
                        header: HeaderLocation::new(element.header().clone(), &agents),
                        op_type: DhtOpType::StoreElement,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                    Op::StoreEntry { header, .. } => {
                        let (with_zome_index, with_entry_def_index) =
                            match header.header.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(zome_id), Some(id)),
                                _ => (None, None),
                            };
                        Event {
                            header: HeaderLocation::new(header.header.clone(), &agents),
                            op_type: DhtOpType::StoreEntry,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterUpdate {
                        update,
                        original_header,
                        ..
                    } => {
                        let (with_zome_index, with_entry_def_index) =
                            match original_header.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(zome_id), Some(id)),
                                _ => (None, None),
                            };
                        Event {
                            header: HeaderLocation::new(update.header.clone(), &agents),
                            op_type: DhtOpType::RegisterUpdatedContent,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterDelete {
                        delete,
                        original_header,
                        ..
                    } => {
                        let (with_zome_index, with_entry_def_index) =
                            match original_header.app_entry_type().cloned() {
                                Some(AppEntryType { id, zome_id, .. }) => (Some(zome_id), Some(id)),
                                _ => (None, None),
                            };
                        Event {
                            header: HeaderLocation::new(delete.header.clone(), &agents),
                            op_type: DhtOpType::RegisterDeletedBy,
                            called_zome: zome,
                            with_zome_index,
                            with_entry_def_index,
                        }
                    }
                    Op::RegisterAgentActivity { header } => Event {
                        header: HeaderLocation::new(header.header().clone(), &agents),
                        op_type: DhtOpType::RegisterAgentActivity,
                        called_zome: zome,
                        with_zome_index: None,
                        with_entry_def_index: None,
                    },
                    Op::RegisterCreateLink { create_link, .. } => Event {
                        header: HeaderLocation::new(create_link.header.clone(), &agents),
                        op_type: DhtOpType::RegisterAddLink,
                        called_zome: zome,
                        with_zome_index: Some(create_link.header.zome_id),
                        with_entry_def_index: None,
                    },
                    Op::RegisterDeleteLink {
                        create_link,
                        delete_link,
                    } => Event {
                        header: HeaderLocation::new(delete_link.header.clone(), &agents),
                        op_type: DhtOpType::RegisterRemoveLink,
                        called_zome: zome,
                        with_zome_index: Some(create_link.zome_id),
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
    let mut zomes = Vec::new();

    let mut agents = HashMap::new();
    agents.insert(alice.clone(), ALICE);
    agents.insert(bob.clone(), BOB);

    zomes.push((
        "zome1",
        InlineZome::new("a", vec![entry_def_a.clone(), entry_def_b.clone()])
            .callback("create_a", call_back_a())
            .callback("create_b", call_back_b())
            .callback(
                "validate",
                validation_callback(ZOME_A_0, agents.clone(), events_tx.clone()),
            ),
    ));
    zomes.push((
        "zome2",
        InlineZome::new("b", vec![entry_def_a.clone(), entry_def_b.clone()])
            .callback("create_a", call_back_a())
            .callback("create_b", call_back_b())
            .callback(
                "validate",
                validation_callback(ZOME_A_1, agents.clone(), events_tx.clone()),
            ),
    ));
    let (dna_file_a, _) = SweetDnaFile::from_inline_zomes("".into(), zomes)
        .await
        .unwrap();
    let mut zomes = Vec::new();
    zomes.push((
        "zome1",
        InlineZome::new("a", vec![entry_def_a.clone(), entry_def_b.clone()])
            .callback("create_a", call_back_a())
            .callback("create_b", call_back_b())
            .callback(
                "validate",
                validation_callback(ZOME_B_0, agents.clone(), events_tx.clone()),
            ),
    ));
    zomes.push((
        "zome2",
        InlineZome::new("b", vec![entry_def_a.clone(), entry_def_b.clone()])
            .callback("create_a", call_back_a())
            .callback("create_b", call_back_b())
            .callback(
                "validate",
                validation_callback(ZOME_B_1, agents.clone(), events_tx.clone()),
            ),
    ));
    let (dna_file_b, _) = SweetDnaFile::from_inline_zomes("".into(), zomes)
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

    let _: HeaderHash = conductors[0]
        .call(&alice.zome("zome1"), "create_a", ())
        .await;

    consistency_10s(&[&alice, &bob]).await;

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

        fn activity_and_element_all_zomes(&mut self, mut event: Event) {
            event.op_type = DhtOpType::RegisterAgentActivity;
            self.all_zomes(event.clone());
            event.op_type = DhtOpType::StoreElement;
            self.all_zomes(event.clone());
        }

        fn zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
            for zome in zomes {
                event.called_zome = *zome;
                self.0.insert(event.clone());
            }
        }

        fn activity_and_element_for_zomes(&mut self, mut event: Event, zomes: &[&'static str]) {
            event.op_type = DhtOpType::RegisterAgentActivity;

            self.zomes(event.clone(), zomes);

            event.op_type = DhtOpType::StoreElement;

            self.zomes(event.clone(), zomes);
        }

        fn genesis(&mut self, agent: &'static str, zomes: &[&'static str]) {
            let event = Event {
                header: HeaderLocation::expect(agent, HeaderType::Dna, 0),
                ..Default::default()
            };
            self.activity_and_element_for_zomes(event.clone(), zomes);

            let event = Event {
                header: HeaderLocation::expect(agent, HeaderType::AgentValidationPkg, 1),
                ..Default::default()
            };
            self.activity_and_element_for_zomes(event.clone(), zomes);

            let mut event = Event {
                header: HeaderLocation::expect(agent, HeaderType::Create, 2),
                ..Default::default()
            };
            self.activity_and_element_for_zomes(event.clone(), zomes);

            event.op_type = DhtOpType::StoreEntry;
            self.zomes(event.clone(), zomes);
        }

        fn init(&mut self, agent: &'static str) {
            let event = Event {
                header: HeaderLocation::expect(agent, HeaderType::InitZomesComplete, 3),
                ..Default::default()
            };
            self.activity_and_element_all_zomes(event.clone());
        }
    }
    let mut expected = Expected(HashSet::new());

    expected.genesis(ALICE, &[ZOME_B_0, ZOME_B_1]);
    expected.genesis(BOB, &[ZOME_A_0, ZOME_A_1]);

    expected.init(ALICE);

    let mut event = Event {
        header: HeaderLocation::expect(ALICE, HeaderType::Create, 4),
        ..Default::default()
    };
    expected.activity_and_element_all_zomes(event.clone());

    event.op_type = DhtOpType::StoreEntry;
    event.called_zome = ZOME_A_0;
    event.with_zome_index = Some(0.into());
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
