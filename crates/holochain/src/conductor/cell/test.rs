use crate::conductor::space::TestSpaces;
use crate::conductor::Conductor;
use crate::core::ribosome::real_ribosome::{
    module_cache::make_module_cache, RealRibosome, WasmBackend,
};
use crate::core::ribosome::Ribosome;
use crate::sweettest::SweetConductorConfig;
use crate::test_utils::fake_valid_dna_file;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_keystore::AgentPubKeyExt;
use holochain_keystore::SignedActionHashedExt;
use holochain_p2p::actor::MockHcP2p;
use holochain_p2p::event::HcP2pHandler;
use holochain_p2p::HolochainP2pDna;
use holochain_state::prelude::*;
use holochain_trace::test_run;
use holochain_types::cell_config_overrides::CellConfigOverrides;
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::test(flavor = "multi_thread")]
async fn direct_signal_receiver_rejects_oversized_payload() {
    use crate::sweettest::{SweetConductor, SweetDnaFile, SweetInlineZomes};
    use holochain_keystore::AgentPubKeyExt;
    use holochain_p2p::event::HcP2pHandler;
    use holochain_types::signal::{
        DirectSignal, DIRECT_SIGNAL_MAX_ENCODED_SIZE, DIRECT_SIGNAL_MAX_SIZE,
    };

    let zomes = SweetInlineZomes::new(vec![], 0).function("grant", |api, (): ()| {
        let action = api.create(CreateInput::new(
            EntryDefLocation::CapGrant,
            EntryVisibility::Private,
            Entry::CapGrant(CapGrant::new_direct_signal_grant(
                "direct-signal".into(),
                GrantConstraint::Unrestricted,
            )),
            ChainTopOrdering::default(),
        ))?;
        Ok(action)
    });
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", &[dna]).await.unwrap();
    let sweet_cell = &app.cells()[0];
    let _: ActionHash = conductor
        .call(&sweet_cell.zome(SweetInlineZomes::COORDINATOR), "grant", ())
        .await;
    let cell = conductor.cell_by_id(sweet_cell.cell_id()).await.unwrap();
    let agent = sweet_cell.agent_pubkey();

    for payload_len in [DIRECT_SIGNAL_MAX_SIZE, DIRECT_SIGNAL_MAX_SIZE + 1] {
        let bytes = holochain_serialized_bytes::encode(&DirectSignal {
            signal: vec![0; payload_len],
            cap_secret: None,
        })
        .unwrap();
        assert!(bytes.len() <= DIRECT_SIGNAL_MAX_ENCODED_SIZE);
        let signature = agent
            .sign_raw(&conductor.keystore(), holo_hash::sha2_512(&bytes).into())
            .await
            .unwrap();
        let result = cell
            .handle_remote_signal_direct(
                sweet_cell.dna_hash().clone(),
                agent.clone(),
                bytes,
                agent.clone(),
                signature,
            )
            .await;
        if payload_len == DIRECT_SIGNAL_MAX_SIZE {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("too long"));
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_handle_publish() {
    test_run();
    let keystore = holochain_keystore::test_keystore();

    let agent_key = keystore.new_sign_keypair_random().await.unwrap();
    let dna_file = fake_valid_dna_file("test_cell_handle_publish");
    let cell_id = CellId::new(dna_file.dna_hash().clone(), agent_key);
    let dna = cell_id.dna_hash().clone();
    let agent = cell_id.agent_pubkey().clone();

    let spaces = TestSpaces::new([dna.clone()]).await;

    let holochain_p2p_cell = HolochainP2pDna::new(Arc::new(MockHcP2p::new()), dna.clone());

    let db_dir = test_db_dir().path().to_path_buf();
    let data_root_path: DataRootPath = db_dir.clone().into();
    let config = SweetConductorConfig::standard().tune_network_config(|nc| {
        nc.disable_bootstrap = true;
    });
    let handle = Conductor::builder()
        .config(config.into())
        .with_keystore(keystore.clone())
        .with_data_root_path(data_root_path.clone())
        .with_unreachable_network()
        .test()
        .await
        .unwrap();
    handle
        .register_dna_file(cell_id.clone(), dna_file.clone())
        .await
        .unwrap();
    let backend = WasmBackend::new();

    let store: WasmStore = WasmStore::test_new();
    let wasmer_module_cache = make_module_cache(backend, store.clone());

    for (hash, wasm) in dna_file.code().clone() {
        store
            .put(DnaWasmHashed::with_pre_hashed(wasm, hash))
            .await
            .unwrap();
    }

    let ribosome = RealRibosome::new(
        backend,
        dna_file.dna_def_hashed().clone(),
        Arc::new(wasmer_module_cache),
    )
    .await
    .unwrap();
    let ribosome = Ribosome::new(dna_file.dna_def_hashed().clone(), ribosome)
        .await
        .unwrap();

    let dht_store = spaces.test_spaces[&dna].space.dht_store.clone();
    super::Cell::genesis(cell_id.clone(), handle.clone(), dht_store, ribosome, None)
        .await
        .unwrap();

    let (_cell, _) = super::Cell::create(
        cell_id,
        handle.clone(),
        spaces.test_spaces[&dna].space.clone(),
        holochain_p2p_cell,
        broadcast::channel(10).0,
        CellConfigOverrides::default(),
    )
    .await
    .unwrap();

    let action = Action {
        header: ActionHeader {
            author: agent.clone(),
            timestamp: Timestamp::now(),
            action_seq: 0,
            prev_action: None,
        },
        data: ActionData::Dna(DnaData {
            dna_hash: dna.clone(),
        }),
    };
    let shh = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(action.clone()),
    )
    .await
    .unwrap();
    let op = DhtOp::ChainOp(Box::new(ChainOp::CreateRecord(
        SignedAction::new(action, shh.signature().clone()),
        OpEntry::ActionOnly,
    )));
    let op_hash = DhtOpHashed::from_content_sync(op.clone()).into_hash();

    spaces
        .spaces
        .handle_publish(&dna, vec![(op, true)])
        .await
        .unwrap();

    // Reading the DhtStore limbo for the published op must not error.
    spaces.test_spaces[&dna]
        .space
        .dht_store
        .as_read()
        .limbo_op_exists(&op_hash)
        .await
        .unwrap();

    handle.shutdown().await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn validation_receipts_require_valid_signatures() {
    test_run();
    let keystore = holochain_keystore::test_keystore();
    let agent = keystore.new_sign_keypair_random().await.unwrap();
    let dna_file = fake_valid_dna_file("validation_receipts_require_valid_signatures");
    let cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());
    let dna = cell_id.dna_hash().clone();
    let spaces = TestSpaces::new([dna.clone()]).await;
    let holochain_p2p_cell = HolochainP2pDna::new(Arc::new(MockHcP2p::new()), dna.clone());
    let data_root_path: DataRootPath = test_db_dir().path().to_path_buf().into();
    let config = SweetConductorConfig::standard().tune_network_config(|nc| {
        nc.disable_bootstrap = true;
    });
    let conductor = Conductor::builder()
        .config(config.into())
        .with_keystore(keystore.clone())
        .with_data_root_path(data_root_path)
        .with_unreachable_network()
        .test()
        .await
        .unwrap();
    conductor
        .register_dna_file(cell_id.clone(), dna_file.clone())
        .await
        .unwrap();

    let backend = WasmBackend::new();
    let store = WasmStore::test_new();
    let wasmer_module_cache = make_module_cache(backend, store.clone());
    for (hash, wasm) in dna_file.code().clone() {
        store
            .put(DnaWasmHashed::with_pre_hashed(wasm, hash))
            .await
            .unwrap();
    }
    let ribosome = RealRibosome::new(
        backend,
        dna_file.dna_def_hashed().clone(),
        Arc::new(wasmer_module_cache),
    )
    .await
    .unwrap();
    let ribosome = Ribosome::new(dna_file.dna_def_hashed().clone(), ribosome)
        .await
        .unwrap();
    let dht_store = spaces.test_spaces[&dna].space.dht_store.clone();
    super::Cell::genesis(
        cell_id.clone(),
        conductor.clone(),
        dht_store.clone(),
        ribosome,
        None,
    )
    .await
    .unwrap();
    let (cell, _) = super::Cell::create(
        cell_id,
        conductor.clone(),
        spaces.test_spaces[&dna].space.clone(),
        holochain_p2p_cell,
        broadcast::channel(10).0,
        CellConfigOverrides::default(),
    )
    .await
    .unwrap();

    // Create an authored op so the cell has something to receive receipts for.
    let action = Action {
        header: ActionHeader {
            author: agent.clone(),
            timestamp: Timestamp::now(),
            action_seq: 0,
            prev_action: None,
        },
        data: ActionData::Dna(DnaData {
            dna_hash: dna.clone(),
        }),
    };
    let signed_action = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(action.clone()),
    )
    .await
    .unwrap();
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::CreateRecord(
        SignedAction::new(action, signed_action.signature().clone()),
        OpEntry::ActionOnly,
    ))));
    let op_hash = op.as_hash().clone();
    dht_store
        .test_insert_authored_chain_op(op, None, None, None, None)
        .await
        .unwrap();

    // Record the publication state before sending a full bundle of forged receipts.
    let publishable_op_count = dht_store
        .as_read()
        .num_still_needing_publish(&agent)
        .await
        .unwrap();
    let validator = keystore.new_sign_keypair_random().await.unwrap();
    let receipts = (0
        ..crate::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE)
        .map(|index| SignedValidationReceipt {
            receipt: ValidationReceipt {
                dht_op_hash: op_hash.clone(),
                validation_status: ValidationStatus::Valid,
                validators: vec![validator.clone()],
                when_integrated: Timestamp::from_micros(index.into()),
            },
            validators_signatures: vec![Signature([0; 64])],
        })
        .collect::<Vec<_>>();

    cell.handle_validation_receipts_received(dna.clone(), agent.clone(), receipts.into())
        .await
        .unwrap();

    // Forged receipts must not be stored or stop the op from being published.
    let stored_receipts = dht_store
        .as_read()
        .validation_receipts_for_op(&op_hash)
        .await
        .unwrap();
    let publishable_op_count_after_receipts = dht_store
        .as_read()
        .num_still_needing_publish(&agent)
        .await
        .unwrap();
    assert_eq!(stored_receipts.len(), 0);
    assert_eq!(publishable_op_count_after_receipts, publishable_op_count);

    // Real signatures should work even when their order differs from the validator list.
    let validator = keystore.new_sign_keypair_random().await.unwrap();
    let second_validator = keystore.new_sign_keypair_random().await.unwrap();

    let valid_content = ValidationReceipt {
        dht_op_hash: op_hash.clone(),
        validation_status: ValidationStatus::Valid,
        validators: vec![validator.clone()],
        when_integrated: Timestamp::from_micros(4),
    };
    let valid_receipt = SignedValidationReceipt {
        validators_signatures: vec![validator
            .sign(&keystore, valid_content.clone())
            .await
            .unwrap()],
        receipt: valid_content,
    };
    let reordered_content = ValidationReceipt {
        dht_op_hash: op_hash.clone(),
        validation_status: ValidationStatus::Valid,
        validators: vec![validator.clone(), second_validator.clone()],
        when_integrated: Timestamp::from_micros(5),
    };
    let reordered_receipt = SignedValidationReceipt {
        validators_signatures: vec![
            second_validator
                .sign(&keystore, reordered_content.clone())
                .await
                .unwrap(),
            validator
                .sign(&keystore, reordered_content.clone())
                .await
                .unwrap(),
        ],
        receipt: reordered_content,
    };

    // Duplicate validators, reused signatures, changed content, and extra signatures are invalid.
    let duplicate_content = ValidationReceipt {
        dht_op_hash: op_hash.clone(),
        validation_status: ValidationStatus::Valid,
        validators: vec![validator.clone(), validator.clone()],
        when_integrated: Timestamp::from_micros(3),
    };
    let duplicate_signature = validator
        .sign(&keystore, duplicate_content.clone())
        .await
        .unwrap();
    let reused_signature = reordered_receipt.validators_signatures[0].clone();
    let mut tampered_receipt = valid_receipt.clone();
    tampered_receipt.receipt.when_integrated = Timestamp::from_micros(6);
    let mut extra_signature_receipt = valid_receipt.clone();
    extra_signature_receipt
        .validators_signatures
        .push(valid_receipt.validators_signatures[0].clone());

    // Mix unsigned and invalid receipts with the valid ones to check what gets stored.
    let receipts = vec![
        SignedValidationReceipt {
            receipt: ValidationReceipt {
                dht_op_hash: op_hash.clone(),
                validation_status: ValidationStatus::Valid,
                validators: Vec::new(),
                when_integrated: Timestamp::from_micros(1),
            },
            validators_signatures: Vec::new(),
        },
        SignedValidationReceipt {
            receipt: ValidationReceipt {
                dht_op_hash: op_hash.clone(),
                validation_status: ValidationStatus::Valid,
                validators: vec![validator.clone()],
                when_integrated: Timestamp::from_micros(2),
            },
            validators_signatures: Vec::new(),
        },
        SignedValidationReceipt {
            receipt: duplicate_content,
            validators_signatures: vec![duplicate_signature.clone(), duplicate_signature],
        },
        SignedValidationReceipt {
            receipt: reordered_receipt.receipt.clone(),
            validators_signatures: vec![reused_signature.clone(), reused_signature],
        },
        extra_signature_receipt,
        tampered_receipt,
        valid_receipt.clone(),
        reordered_receipt.clone(),
    ];

    cell.handle_validation_receipts_received(dna, agent, receipts.into())
        .await
        .unwrap();

    // Only the two correctly signed receipts should have been stored.
    let stored_receipts = dht_store
        .as_read()
        .validation_receipts_for_op(&op_hash)
        .await
        .unwrap();
    assert_eq!(stored_receipts, vec![valid_receipt, reordered_receipt]);

    conductor.shutdown().await.unwrap().unwrap();
}
