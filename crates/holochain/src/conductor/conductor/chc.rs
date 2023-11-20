use holochain_p2p::ChcImpl;

use super::*;

impl Conductor {
    #[allow(unused_variables)]
    pub(crate) fn chc(&self, keystore: MetaLairClient, cell_id: &CellId) -> Option<ChcImpl> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "chc")] {
                crate::conductor::chc::build_chc(self.config.chc_url.as_ref().map(|u| u.as_ref()), keystore, cell_id)
            } else {
                None
            }
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) async fn chc_sync(
        self: Arc<Self>,
        cell_id: CellId,
        enable_app: Option<InstalledAppId>,
    ) -> ConductorApiResult<()> {
        if let Some(chc) = self.chc(self.keystore().clone(), &cell_id) {
            let db = self.get_authored_db(cell_id.dna_hash())?;
            let author = cell_id.agent_pubkey().clone();
            let top_hash = db
                .read_async(move |txn| {
                    SourceChainResult::Ok(chain_head_db(&txn, Arc::new(author))?.map(|h| h.action))
                })
                .await?;
            let records = chc.get_record_data(top_hash).await?;

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
