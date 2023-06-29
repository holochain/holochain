use std::sync::Arc;

use holochain_keystore::MetaLairClient;
use holochain_types::prelude::*;

use crate::core::validate_chain;

/// Mutable wrapper around local CHC
pub struct ChcLocal<A: ChainItem = SignedActionHashed> {
    inner: parking_lot::Mutex<ChcLocalInner<A>>,
    keystore: MetaLairClient,
    agent: AgentPubKey,
}

impl<A: ChainItem> ChcLocal<A> {
    /// Constructor
    pub fn new(keystore: MetaLairClient, agent: AgentPubKey) -> Self {
        Self {
            inner: parking_lot::Mutex::new(Default::default()),
            keystore,
            agent,
        }
    }
}

/// A local Rust implementation of a CHC, for testing purposes only.
pub struct ChcLocalInner<A: ChainItem = SignedActionHashed> {
    records: Vec<AddRecordPayload<A>>,
}

impl<A: ChainItem> Default for ChcLocalInner<A> {
    fn default() -> Self {
        Self {
            records: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcLocal {
    type Item = SignedActionHashed;

    async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()> {
        let mut m = self.inner.lock();
        let head = m
            .records
            .last()
            .map(|r| (r.action.get_hash().clone(), r.action.seq()));
        let actions = request.iter().map(|r| &r.action);
        validate_chain(actions, &head).map_err(|_| {
            let (hash, seq) = head.unwrap();
            ChcError::InvalidChain(seq, hash)
        })?;
        m.records.extend(request);
        Ok(())
    }

    async fn get_record_data_request(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>> {
        let m = self.inner.lock();
        let records = if let Some(hash) = request.payload.since_hash.as_ref() {
            m.records
                .iter()
                .skip_while(|r| hash != r.action.get_hash())
                .skip(1)
                .cloned()
                .collect()
        } else {
            m.records.clone()
        };
        Ok(records
            .into_iter()
            .map(
                |AddRecordPayload {
                     action,
                     encrypted_entry,
                 }| (action, encrypted_entry),
            )
            .collect())
    }
}

impl ChainHeadCoordinatorExt for ChcLocal {
    fn signing_info(&self) -> (MetaLairClient, AgentPubKey) {
        (self.keystore.clone(), self.agent.clone())
    }
}

#[cfg(test)]
mod tests {
    use holochain_conductor_api::conductor::ConductorConfig;
    use isotest::Iso;

    use crate::{
        conductor::chc::{ChcRemote, CHC_LOCAL_MAGIC_URL, CHC_LOCAL_MAP},
        sweettest::*,
        test_utils::valid_arbitrary_chain,
    };

    use super::*;
    use ChainHeadCoordinatorExt;

    use ::fixt::prelude::*;
    use holochain_types::test_utils::chain::{TestChainHash, TestChainItem};

    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_records_local() {
        let keystore = test_keystore();
        let agent = fake_agent_pubkey_1();
        let chc = Arc::new(ChcLocal::new(keystore, agent));

        assert_eq!(chc.clone().head().await.unwrap(), None);

        let hash = |x| TestChainHash(x).real();
        let item = |x| Record::new(TestChainItem::new(x).real(), None);

        let items = |i: &[u32]| i.into_iter().copied().map(item).collect::<Vec<_>>();

        let t0 = items(&[0, 1, 2]);
        let t1 = items(&[3, 4, 5]);
        let t2 = items(&[6, 7, 8]);
        let t99 = items(&[99]);

        chc.clone().add_records(t0.clone()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(2));
        chc.clone().add_records(t1.clone()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        // last_hash doesn't match
        assert!(chc.clone().add_records(t0.clone()).await.is_err());
        assert!(chc.clone().add_records(t1.clone()).await.is_err());
        assert!(chc.clone().add_records(t99).await.is_err());
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        chc.clone().add_records(t2.clone()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(8));

        assert_eq!(
            chc.clone().get_record_data(None).await.unwrap(),
            items(&[0, 1, 2, 3, 4, 5, 6, 7, 8])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(0))).await.unwrap(),
            items(&[1, 2, 3, 4, 5, 6, 7, 8])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(3))).await.unwrap(),
            items(&[4, 5, 6, 7, 8])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(7))).await.unwrap(),
            items(&[8])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(8))).await.unwrap(),
            items(&[])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(9))).await.unwrap(),
            items(&[])
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(33))).await.unwrap(),
            items(&[])
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn simple_chc_sync() {
        use holochain::test_utils::inline_zomes::simple_crud_zome;

        let mut config = ConductorConfig::default();
        config.chc_url = Some(url2::Url2::parse(CHC_LOCAL_MAGIC_URL));
        let mut conductor = SweetConductor::from_config(config).await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
        let (agent, _) = SweetAgents::alice_and_bob();

        let (cell,) = conductor
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap()
            .into_tuple();

        let cell_id = cell.cell_id();

        let top_hash = {
            let mut dump = conductor
                .dump_full_cell_state(&cell_id, None)
                .await
                .unwrap();
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
            let m = CHC_LOCAL_MAP.lock();
            let chc = m.get(&cell_id).unwrap();
            let records = chc.clone().get_record_data(None).await.unwrap();
            assert_eq!(records.len(), 3);
            chc.clone().add_records(vec![new_record]).await.unwrap();
        }

        // Check that a sync picks up the new action
        conductor
            .raw_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump = conductor
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();
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

    #[tokio::test(flavor = "multi_thread")]
    async fn multi_conductor_chc_sync() {
        use holochain::test_utils::inline_zomes::{simple_crud_zome, AppString};

        let mut config = ConductorConfig::default();
        // config.chc_url = Some(url2::Url2::parse("http://127.0.0.1:40845/v1/"));
        config.chc_url = Some(url2::Url2::parse(CHC_LOCAL_MAGIC_URL));
        let mut conductors =
            SweetConductorBatch::from_configs([config.clone(), config.clone(), config.clone()])
                .await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
        let (agent, _) = SweetAgents::alice_and_bob();

        let (c0,) = conductors[0]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap()
            .into_tuple();

        let cell_id = c0.cell_id();

        let install_result_1 = conductors[1]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await;
        let install_result_2 = conductors[2]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await;

        // It's not ideal to match on a string, but it seems like the only option:
        // - The pattern involves Boxes which are impossible to match on
        // - The error types are not PartialEq, so cannot be constructed and tested for equality

        dbg!(&install_result_1);
        dbg!(&install_result_2);

        regex::Regex::new(
            r#".*ChcHeadMoved\("genesis", InvalidChain\(2, ActionHash\([a-zA-Z0-9-_]+\)\)\).*"#,
        )
        .unwrap()
        .captures(&format!("{:?}", install_result_1))
        .unwrap();
        assert_eq!(
            format!("{:?}", install_result_1),
            format!("{:?}", install_result_2)
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

        let dump1 = conductors[1]
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();

        assert_eq!(dump1.source_chain_dump.records.len(), 3);

        let _apps1 = conductors[1]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap();
        let _apps2 = conductors[2]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap();

        let c1: SweetCell = conductors[1].get_sweet_cell(cell_id.clone()).unwrap();
        let c2: SweetCell = conductors[2].get_sweet_cell(cell_id.clone()).unwrap();

        let _: ActionHash = conductors[0]
            .call(
                &c0.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                AppString::new("zero"),
            )
            .await;

        // This should fail and require triggering a CHC sync
        let hash1: Result<ActionHash, _> = conductors[1]
            .call_fallible(
                &c1.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                AppString::new("one"),
            )
            .await;

        dbg!(&hash1);

        regex::Regex::new(
            r#".*ChcHeadMoved\("SourceChain::flush", InvalidChain\(4, ActionHash\([a-zA-Z0-9-_]+\).*"#
        ).unwrap().captures(&format!("{:?}", hash1)).unwrap();

        // This should trigger a CHC sync
        let hash2: Result<ActionHash, _> = conductors[2]
            .call_fallible(
                &c2.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                AppString::new("two"),
            )
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
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();
        let dump1 = conductors[1]
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();
        let dump2 = conductors[2]
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();

        assert_eq!(dump0.source_chain_dump.records.len(), 5);
        assert_eq!(
            dump0.source_chain_dump.records,
            dump1.source_chain_dump.records
        );
        assert_eq!(
            dump1.source_chain_dump.records,
            dump2.source_chain_dump.records
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "this test requires a remote service, so it should only be run manually"]
    async fn test_add_records_remote() {
        let keystore = test_keystore();
        let agent = fake_agent_pubkey_1();
        let cell_id = CellId::new(fixt!(DnaHash), agent.clone());
        let chc = Arc::new(ChcRemote::new(
            url::Url::parse("http://127.0.0.1:40845/v1/").unwrap(),
            // url::Url::parse("https://chc.dev.holotest.net/v1/").unwrap(),
            keystore.clone(),
            &cell_id,
        ));

        let mut g = random_generator();

        let chain = valid_arbitrary_chain(&mut g, keystore, agent, 20).await;

        let t0 = &chain[0..3];
        let t1 = &chain[3..6];
        let t2 = &chain[6..9];
        let t11 = &chain[11..=11];

        let hash = |i: usize| chain[i].action_address().clone();

        chc.clone().add_records(t0.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(2));

        chc.clone().add_records(t1.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        // last_hash doesn't match
        assert!(chc.clone().add_records(t0.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t1.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t11.to_vec()).await.is_err());
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        chc.clone().add_records(t2.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(8));

        assert_eq!(
            chc.clone().get_record_data(None).await.unwrap(),
            &chain[0..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(0))).await.unwrap(),
            &chain[1..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(3))).await.unwrap(),
            &chain[4..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(7))).await.unwrap(),
            &chain[8..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(8))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(9))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(13))).await.unwrap(),
            &[]
        );
    }
}
