use std::collections::{HashMap, HashSet};

use holochain_types::prelude::*;

use crate::core::validate_chain;

/// Mutable wrapper around local CHC
pub struct ChcLocal<A: ChainItem = SignedActionHashed>(parking_lot::Mutex<ChcLocalInner<A>>);

impl<A: ChainItem> ChcLocal<A> {
    /// Constructor
    pub fn new() -> Self {
        Self(parking_lot::Mutex::new(Default::default()))
    }
}

/// A local Rust implementation of a CHC, for testing purposes only.
pub struct ChcLocalInner<A: ChainItem = SignedActionHashed> {
    actions: Vec<A>,
    entries: HashMap<EntryHash, Entry>,
}

impl<A: ChainItem> Default for ChcLocalInner<A> {
    fn default() -> Self {
        Self {
            actions: Default::default(),
            entries: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl<A: ChainItem> ChainHeadCoordinator for ChcLocal<A> {
    type Item = A;

    async fn head(&self) -> ChcResult<Option<A::Hash>> {
        Ok(self.0.lock().actions.last().map(|a| a.get_hash().clone()))
    }

    async fn add_actions(&self, new_actions: Vec<A>) -> ChcResult<()> {
        let mut m = self.0.lock();
        let head = m.actions.last().map(|a| (a.get_hash().clone(), a.seq()));
        let seq = head.as_ref().map(|h| h.1);
        validate_chain(new_actions.iter(), &head)
            .map_err(|e| ChcError::InvalidChain(seq, e.to_string()))?;
        m.actions.extend(new_actions);
        Ok(())
    }

    async fn add_entries(&self, entries: Vec<EntryHashed>) -> ChcResult<()> {
        let mut m = self.0.lock();
        m.entries
            .extend(entries.into_iter().map(|e| swap2(e.into_inner())));
        Ok(())
    }

    async fn get_actions_since_hash(&self, hash: Option<A::Hash>) -> ChcResult<Vec<A>> {
        let m = self.0.lock();
        let result = if let Some(hash) = hash.as_ref() {
            let mut actions = m.actions.iter().skip_while(|a| hash != a.get_hash());

            if actions.next().is_none() {
                m.actions.clone()
            } else {
                actions.cloned().collect()
            }
        } else {
            m.actions.clone()
        };
        Ok(result)
    }

    async fn get_entries(
        &self,
        mut hashes: HashSet<&EntryHash>,
    ) -> ChcResult<HashMap<EntryHash, Entry>> {
        let m = self.0.lock();
        let entries = m
            .entries
            .iter()
            .filter_map(|(h, e)| {
                hashes.contains(h).then(|| {
                    hashes.remove(h);
                    (h.clone(), e.clone())
                })
            })
            .collect();
        if !hashes.is_empty() {
            Err(ChcError::MissingEntries(
                hashes.into_iter().cloned().collect(),
            ))
        } else {
            Ok(entries)
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;
    use holochain_state::prelude::SourceChainError;
    use matches::assert_matches;

    use holochain_conductor_api::conductor::ConductorConfig;

    use crate::{
        conductor::{
            api::error::ConductorApiError, chc::CHC_LOCAL_MAP, error::ConductorError, CellError,
        },
        core::workflow::error::WorkflowError,
        sweettest::*,
    };

    use super::*;

    use ::fixt::prelude::*;
    use holochain_types::{
        fixt::*,
        test_utils::chain::{TestChainHash, TestChainItem},
    };

    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_actions() {
        isotest::isotest_async!(TestChainItem, TestChainHash => |iso_a, iso_h| async move {
            let chc = ChcLocal::new();
            assert_eq!(chc.head().await.unwrap(), None);

            let hash = |x| iso_h.create(TestChainHash(x));
            let item = |x| iso_a.create(TestChainItem::new(x));

            let items = |i: &[u32]| i.into_iter().copied().map(item).collect::<Vec<_>>();

            let t0 = items(&[0, 1, 2]);
            let t1 = items(&[3, 4, 5]);
            let t2 = items(&[6, 7, 8]);
            let t99 = items(&[99]);

            chc.add_actions(t0.clone()).await.unwrap();
            assert_eq!(chc.head().await.unwrap().unwrap(), hash(2));
            chc.add_actions(t1.clone()).await.unwrap();
            assert_eq!(chc.head().await.unwrap().unwrap(), hash(5));

            // last_hash doesn't match
            assert!(chc.add_actions(t0.clone()).await.is_err());
            assert!(chc.add_actions(t1.clone()).await.is_err());
            assert!(chc.add_actions(t99).await.is_err());
            assert_eq!(chc.head().await.unwrap().unwrap(), hash(5));

            chc.add_actions(t2.clone()).await.unwrap();
            assert_eq!(chc.head().await.unwrap().unwrap(), hash(8));

            assert_eq!(
                chc.get_actions_since_hash(None).await.unwrap(),
                items(&[0, 1, 2, 3, 4, 5, 6, 7, 8])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(0))).await.unwrap(),
                items(&[1, 2, 3, 4, 5, 6, 7, 8])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(3))).await.unwrap(),
                items(&[4, 5, 6, 7, 8])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(7))).await.unwrap(),
                items(&[8])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(8))).await.unwrap(),
                items(&[])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(9))).await.unwrap(),
                items(&[0, 1, 2, 3, 4, 5, 6, 7, 8])
            );
            assert_eq!(
                chc.get_actions_since_hash(Some(hash(33))).await.unwrap(),
                items(&[0, 1, 2, 3, 4, 5, 6, 7, 8])
            );
        }
        .boxed());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn simple_chc_sync() {
        use holochain::test_utils::inline_zomes::{simple_crud_zome};

        let mut config = ConductorConfig::default();
        config.chc_namespace = Some("#LOCAL#".to_string());
        let mut conductor = SweetConductor::from_config(config).await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome())
            .await
            .unwrap();
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
            dbg!(&dump.source_chain_dump);
            assert_eq!(dump.source_chain_dump.records.len(), 3);
            dump.source_chain_dump.records.pop().unwrap().action_address
        };

        let new_entry = EntryHashed::from_content_sync(fixt!(Entry));
        let new_entry_hash = new_entry.as_hash().clone();
        dbg!(&new_entry_hash);
        let create = Create {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(9999999999),
            action_seq: 3,
            prev_action: top_hash,
            entry_type: fixt!(EntryType),
            entry_hash: new_entry_hash,
            weight: EntryRateWeight::default(),
        };
        let new_action = ActionHashed::from_content_sync(Action::Create(create));
        // *new_action.prev_action_mut().unwrap() = top_hash;
        // *new_action.action_seq_mut().unwrap() = 3;
        // *new_action.author_mut() = agent.clone();
        // *new_action.entry_data_mut().unwrap().0 = new_entry.as_hash().clone();
        let new_action = SignedActionHashed::sign(&conductor.keystore(), new_action)
            .await
            .unwrap();

        {
            // add some data to the local CHC
            let m = CHC_LOCAL_MAP.lock();
            let chc = m.get(&cell_id).unwrap();
            let actions = chc.get_actions_since_hash(None).await.unwrap();
            assert_eq!(actions.len(), 3);
            chc.add_actions(vec![new_action]).await.unwrap();
            chc.add_entries(vec![new_entry]).await.unwrap();
        }

        // Check that a sync picks up the new action
        conductor
            .inner_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump = conductor
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();
        dbg!(&dump);
        assert_eq!(dump.source_chain_dump.records.len(), 4);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multi_conductor_chc_sync() {
        use holochain::test_utils::inline_zomes::{simple_crud_zome, AppString};

        let mut config = ConductorConfig::default();
        config.chc_namespace = Some("#LOCAL#".to_string());
        let mut conductors =
            SweetConductorBatch::from_configs([config.clone(), config.clone(), config.clone()])
                .await;

        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome())
            .await
            .unwrap();
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
        assert_eq!(
            format!("{:?}", install_result_1),
            r#"Err(ConductorError(GenesisFailed { errors: [ConductorApiError(WorkflowError(SourceChainError(ChcHeadMoved("genesis", InvalidChain(Some(2), "Action is not the first, so needs previous action")))))] }))"#
        );
        assert_eq!(
            format!("{:?}", install_result_1),
            format!("{:?}", install_result_2)
        );

        // TODO: sync conductors 1 and 2 to match conductor 0
        conductors[1]
            .inner_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();
        conductors[2]
            .inner_handle()
            .chc_sync(cell_id.clone(), None)
            .await
            .unwrap();

        let dump1 = conductors[1]
            .dump_full_cell_state(&cell_id, None)
            .await
            .unwrap();
        dbg!(&dump1);
        assert_eq!(dump1.source_chain_dump.records.len(), 3);

        let install_result_1 = conductors[1]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap();
        let install_result_2 = conductors[2]
            .setup_app_for_agent("app", agent.clone(), [&dna_file])
            .await
            .unwrap();

        let c1: SweetCell = conductors[1].get_sweet_cell(cell_id.clone()).unwrap();
        let c2: SweetCell = conductors[2].get_sweet_cell(cell_id.clone()).unwrap();

        let hash0: ActionHash = conductors[0]
            .call(
                &c0.zome(SweetEasyInline::COORDINATOR),
                "create_string",
                AppString::new("zero"),
            )
            .await;

        // This should fail and require triggering a CHC sync
        let hash1: Result<ActionHash, _> = conductors[1]
            .call_fallible(
                &c1.zome(SweetEasyInline::COORDINATOR),
                "create_string",
                AppString::new("one"),
            )
            .await;

        assert_matches!(
            hash1,
            Err(ConductorApiError::SourceChainError(SourceChainError::ChcHeadMoved(_, ChcError::InvalidChain(seq, _))))
            if seq == Some(0)
        );

        // This should trigger a CHC sync
        let hash2: Result<ActionHash, _> = conductors[2]
            .call_fallible(
                &c2.zome(SweetEasyInline::COORDINATOR),
                "create_string",
                AppString::new("two"),
            )
            .await;

        assert_matches!(
            hash2,
            Err(ConductorApiError::SourceChainError(SourceChainError::ChcHeadMoved(_, ChcError::InvalidChain(seq, _))))
            if seq == Some(0)
        );
    }
}
