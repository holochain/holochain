use holochain_p2p::ChcImpl;

use super::*;

impl Conductor {
    #[allow(unused_variables)]
    pub(crate) fn chc(&self, cell_id: &CellId) -> Option<ChcImpl> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "chc")] {
                crate::conductor::chc::build_chc(self.config.chc_namespace.as_ref(), cell_id)
            } else {
                None
            }
        }
    }

    #[cfg(any(test))]
    #[allow(dead_code)]
    pub(crate) async fn chc_sync(
        self: Arc<Self>,
        cell_id: CellId,
        enable_app: Option<InstalledAppId>,
    ) -> ConductorApiResult<()> {
        if let Some(chc) = self.chc(&cell_id) {
            let db = self.get_authored_db(cell_id.dna_hash())?;
            let author = cell_id.agent_pubkey().clone();
            let top_hash = db
                .async_reader(move |txn| {
                    SourceChainResult::Ok(chain_head_db(&txn, Arc::new(author))?.map(|h| h.action))
                })
                .await?;
            let actions = chc.get_actions_since_hash(top_hash).await?;
            let entry_hashes: HashSet<&EntryHash> = actions
                .iter()
                .filter_map(|a| a.hashed.entry_hash())
                .collect();
            dbg!(&actions, &entry_hashes);
            let entries = chc.get_entries(entry_hashes).await?;
            dbg!(&entries);
            let records = records_from_actions_and_entries(actions, entries)?;
            dbg!(&records);

            self.clone()
                .graft_records_onto_source_chain(cell_id, true, records)
                .await?;
            if let Some(app_id) = enable_app {
                self.enable_app(app_id).await?;
            }
        }
        Ok(())
    }
}
