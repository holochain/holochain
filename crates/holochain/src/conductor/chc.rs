//! Types for Chain Head Coordination

use holochain_keystore::MetaLairClient;
use holochain_zome_types::prelude::*;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};
use url::Url;

pub use holochain_chc::*;

/// Storage for the local CHC implementations
pub static CHC_LOCAL_MAP: Lazy<parking_lot::Mutex<HashMap<CellId, ChcImpl>>> =
    Lazy::new(|| parking_lot::Mutex::new(HashMap::new()));

/// The URL which indicates that the fake local CHC service should be used,
/// instead of a remote service via HTTP
pub const CHC_LOCAL_MAGIC_URL: &str = "local:";

/// Build the appropriate CHC implementation.
///
/// In particular, if the url is the magic string "local:", then a [`ChcLocal`]
/// implementation will be used. Otherwise, if the url is set, and the CellId
/// is "CHC-enabled", then a [`ChcRemote`] will be produced.
pub fn build_chc(
    base_url: Option<&Url>,
    keystore: MetaLairClient,
    cell_id: &CellId,
) -> Option<ChcImpl> {
    // TODO: check if the agent key is Holo-hosted, otherwise return none
    let is_holo_agent = true;
    if is_holo_agent {
        base_url.map(|url| {
            #[cfg(feature = "chc")]
            {
                fn chc_local(keystore: MetaLairClient, cell_id: CellId) -> ChcImpl {
                    let agent = cell_id.agent_pubkey().clone();
                    let mut m = CHC_LOCAL_MAP.lock();
                    m.entry(cell_id)
                        .or_insert_with(|| Arc::new(chc_local::ChcLocal::new(keystore, agent)))
                        .clone()
                }

                fn chc_remote(
                    base_url: Url,
                    keystore: MetaLairClient,
                    cell_id: &CellId,
                ) -> ChcImpl {
                    Arc::new(chc_http::ChcHttp::new(base_url, keystore, cell_id))
                }

                if url.as_str() == CHC_LOCAL_MAGIC_URL {
                    chc_local(keystore, cell_id.clone())
                } else {
                    chc_remote(url.clone(), keystore, cell_id)
                }
            }

            #[cfg(not(feature = "chc"))]
            panic!("CHC is not enabled in this build. Rebuild with the `chc` feature enabled.")
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {

    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::{atomic::AtomicBool, Arc};

    use hdk::prelude::*;
    use holochain_chc::*;
    use holochain_conductor_api::conductor::{ConductorConfig, DpkiConfig};
    use holochain_keystore::MetaLairClient;
    use holochain_state::prelude::SourceChainError;
    use holochain_types::record::SignedActionHashedExt;
    use holochain_wasm_test_utils::TestWasm;

    use crate::conductor::CellError;
    use crate::core::workflow::WorkflowError;
    use crate::{
        conductor::{
            api::error::ConductorApiError,
            chc::{CHC_LOCAL_MAGIC_URL, CHC_LOCAL_MAP},
            error::ConductorError,
        },
        sweettest::*,
    };

    /// A CHC implementation that can be set up to error
    struct FlakyChc {
        chc: chc_local::ChcLocal,
        agent: AgentPubKey,
        keystore: MetaLairClient,
        pub fail: AtomicBool,
    }

    #[async_trait::async_trait]
    impl ChainHeadCoordinator for FlakyChc {
        type Item = SignedActionHashed;

        async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()> {
            if self.fail.load(SeqCst) {
                Err(ChcError::Other("bad".to_string()))
            } else {
                self.chc.add_records_request(request).await
            }
        }

        async fn get_record_data_request(
            &self,
            request: GetRecordsRequest,
        ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>>
        {
            if self.fail.load(SeqCst) {
                Err(ChcError::Other("bad".to_string()))
            } else {
                self.chc.get_record_data_request(request).await
            }
        }
    }

    impl ChainHeadCoordinatorExt for FlakyChc {
        fn signing_info(&self) -> (MetaLairClient, AgentPubKey) {
            (self.keystore.clone(), self.agent.clone())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn simple_chc_sync() {
        use holochain::test_utils::inline_zomes::simple_crud_zome;

        let config = ConductorConfig {
            chc_url: Some(url2::Url2::parse(CHC_LOCAL_MAGIC_URL)),
            ..Default::default()
        };
        let mut conductor = SweetConductor::standard().await.local_chc();

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

        let (cell,) = conductor
            .setup_app("app", &[dna_file])
            .await
            .unwrap()
            .into_tuple();

        let cell_id = cell.cell_id();
        let agent = cell_id.agent_pubkey().clone();

        let top_hash = {
            let mut dump = conductor.dump_full_cell_state(cell_id, None).await.unwrap();
            assert_eq!(dump.source_chain_dump.records.len(), 3);
            dump.source_chain_dump.records.pop().unwrap().action_address
        };

        let izc = InitZomesComplete {
            author: agent.clone(),
            timestamp: Timestamp::now(),
            action_seq: 3,
            prev_action: top_hash,
        };
        let new_action = ActionHashed::from_content_sync(Action::InitZomesComplete(izc));
        let new_action = SignedActionHashed::sign(&conductor.keystore(), new_action)
            .await
            .unwrap();
        let new_action_hash = new_action.action_address().clone();
        let new_record = Record::new(new_action, None);

        {
            // add some data to the local CHC
            let chc = CHC_LOCAL_MAP.lock().get(cell_id).unwrap().clone();
            let records = chc.clone().get_record_data(None).await.unwrap();
            assert_eq!(records.len(), 3);
            chc.add_records(vec![new_record]).await.unwrap();
        }

        // Check that a sync picks up the new action
        conductor
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump = conductor.dump_full_cell_state(cell_id, None).await.unwrap();
        assert_eq!(dump.source_chain_dump.records.len(), 4);
        assert_eq!(
            dump.source_chain_dump
                .records
                .last()
                .unwrap()
                .action_address,
            new_action_hash,
        );
    }

    /// Test that general CHC failures prevent chain writes
    #[tokio::test(flavor = "multi_thread")]
    async fn simple_chc_error_prevents_write() {
        holochain_trace::test_run();
        use holochain::test_utils::inline_zomes::simple_crud_zome;

        let config = ConductorConfig {
            chc_url: Some(url2::Url2::parse(CHC_LOCAL_MAGIC_URL)),
            dpki: DpkiConfig::disabled(),
            ..Default::default()
        };
        let mut conductor = SweetConductor::from_config(config).await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
        let agent = SweetAgents::alice();
        let cell_id = CellId::new(dna_file.dna_hash().clone(), agent.clone());

        let flaky_chc = Arc::new(FlakyChc {
            chc: chc_local::ChcLocal::new(conductor.keystore(), agent.clone()),
            keystore: conductor.keystore().clone(),
            agent: agent.clone(),
            fail: true.into(),
        });

        // Set up the flaky CHC ahead of time
        CHC_LOCAL_MAP
            .lock()
            .insert(cell_id.clone(), flaky_chc.clone());

        // The app can't be installed, because of a CHC error during genesis
        let err = conductor
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap_err();
        matches::assert_matches!(
            err,
            ConductorApiError::ConductorError(ConductorError::GenesisFailed { .. })
        );

        // Make the CHC work again
        flaky_chc.fail.store(false, SeqCst);

        // Genesis can now complete
        let (cell,) = conductor
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap()
            .into_tuple();

        // Make the CHC fail again
        flaky_chc.fail.store(true, SeqCst);

        // A zome call can't be made, because of a CHC error
        let err = conductor
            .call_fallible::<_, ActionHash>(&cell.zome("coordinator"), "create_unit", ())
            .await
            .unwrap_err();

        matches::assert_matches!(
            err,
            ConductorApiError::CellError(CellError::WorkflowError(we))
            if matches!(*we, WorkflowError::SourceChainError(SourceChainError::Other(_)))
        );
    }

    // TODO: run this against a remote CHC too
    #[tokio::test(flavor = "multi_thread")]
    async fn multi_conductor_chc_sync() {
        holochain_trace::test_run();

        let mut config = SweetConductorConfig::standard().no_dpki();
        // config.chc_url = Some(url2::Url2::parse("http://127.0.0.1:40845/"));
        config.chc_url = Some(url2::Url2::parse(CHC_LOCAL_MAGIC_URL));
        let mut conductors = SweetConductorBatch::from_config(4, config).await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

        // All conductors share the same known agent, already installed in the test_keystore
        let agent = SweetAgents::alice();

        let (c0,) = conductors[0]
            .setup_app_for_agent("app", agent.clone(), &[dna_file.clone()])
            .await
            .unwrap()
            .into_tuple();

        let cell_id = c0.cell_id();

        // Install two apps with ignore_genesis_failure and one without
        let mk_payload = |ignore: bool| {
            let agent = agent.clone();
            let dna_file = dna_file.clone();
            async move {
                let mut payload = get_install_app_payload_from_dnas(
                    "app",
                    Some(agent),
                    &[(dna_file, None)],
                    None,
                )
                .await;
                payload.ignore_genesis_failure = ignore;
                payload
            }
        };

        let install_result_1 = conductors[1]
            .raw_handle()
            .install_app_bundle(mk_payload(true).await)
            .await;
        let install_result_2 = conductors[2]
            .raw_handle()
            .install_app_bundle(mk_payload(true).await)
            .await;
        let install_result_3 = conductors[3]
            .raw_handle()
            .install_app_bundle(mk_payload(false).await)
            .await;

        // It's not ideal to match on a string, but it seems like the only sane option:
        // - The pattern involves Boxes which require multiple steps for matching
        // - The error actually contains a Vec of errors
        // - The innermost error is actually a SourceChainError::Other, which is a boxed trait object, not matchable
        // - The error types are not PartialEq, so cannot be constructed and tested for equality
        // Here's the closest attempt, which does not work (Needs to use SourceChainError::Other), and perhaps a downcast:
        /*
                if let Err(ConductorError::GenesisFailed { errors }) = &install_result_1 {
                    assert_eq!(errors.len(), 1);
                    if let CellError::ConductorApiError(b) = &errors[0].1 {
                        assert_matches!(
                            &**b,
                            ConductorApiError::WorkflowError(WorkflowError::SourceChainError(SourceChainError::ChcError(
                                ChcError::InvalidChain(seq, _)
                            )))
                            if *seq == 2
                        );
                    }
                }
        */

        println!("install_result_1 = {:?}", install_result_1);
        println!("install_result_2 = {:?}", install_result_2);
        println!("install_result_3 = {:?}", install_result_3);

        regex::Regex::new(r#".*ChcError\(InvalidChain\((\d+), ActionHash\([a-zA-Z0-9-_]+\)\)\).*"#)
            .unwrap()
            .captures(&format!("{:?}", install_result_1))
            .unwrap();
        // TODO: check sequence and hash

        assert_eq!(
            format!("{:?}", install_result_1),
            format!("{:?}", install_result_2)
        );
        assert_eq!(
            format!("{:?}", install_result_2),
            format!("{:?}", install_result_3)
        );

        assert!(conductors[1]
            .get_app_info(&"app".into())
            .await
            .unwrap()
            .is_some());
        assert!(conductors[2]
            .get_app_info(&"app".into())
            .await
            .unwrap()
            .is_some());

        // This one will not have app info, since it was installed without `ignore_genesis_failure`
        assert_eq!(
            conductors[3].get_app_info(&"app".into()).await.unwrap(),
            None
        );

        // TODO: sync conductors 1 and 2 to match conductor 0
        conductors[1]
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();
        conductors[2]
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        // Sync is possible even though installation was rolled back and the cell was removed
        conductors[3]
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump1 = conductors[1]
            .dump_full_cell_state(cell_id, None)
            .await
            .unwrap();

        assert_eq!(dump1.source_chain_dump.records.len(), 3);

        let c1: SweetCell = conductors[1].get_sweet_cell(cell_id.clone()).unwrap();
        let c2: SweetCell = conductors[2].get_sweet_cell(cell_id.clone()).unwrap();

        let _: ActionHash = conductors[0]
            .call(&c0.zome(TestWasm::Create), "create_entry", ())
            .await;

        conductors[1].enable_app("app".into()).await.unwrap();
        conductors[2].enable_app("app".into()).await.unwrap();

        // This should fail and require triggering a CHC sync
        let hash1: Result<ActionHash, _> = conductors[1]
            .call_fallible(&c1.zome(TestWasm::Create), "create_entry", ())
            .await;

        dbg!(&hash1);

        regex::Regex::new(r#".*ChcError\(InvalidChain\((\d+), ActionHash\([a-zA-Z0-9-_]+\).*"#)
            .unwrap()
            .captures(&format!("{:?}", hash1))
            .unwrap();
        // TODO: check sequence and hash

        // This should trigger a CHC sync
        let hash2: Result<ActionHash, _> = conductors[2]
            .call_fallible(&c2.zome(TestWasm::Create), "create_entry", ())
            .await;

        assert_eq!(format!("{:?}", hash1), format!("{:?}", hash2));

        conductors[1]
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        conductors[2]
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump0 = conductors[0]
            .dump_full_cell_state(cell_id, None)
            .await
            .unwrap();
        let dump1 = conductors[1]
            .dump_full_cell_state(cell_id, None)
            .await
            .unwrap();
        let dump2 = conductors[2]
            .dump_full_cell_state(cell_id, None)
            .await
            .unwrap();

        assert_eq!(dump0.source_chain_dump.records.len(), 6);
        assert_eq!(
            dump0.source_chain_dump.records,
            dump1.source_chain_dump.records
        );
        assert_eq!(
            dump1.source_chain_dump.records,
            dump2.source_chain_dump.records
        );
    }
}
