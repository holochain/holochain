use holochain_chc::ChcImpl;

use super::*;

impl Conductor {
    /// Get access to the CHC used by the conductor
    #[allow(unused_variables)]
    pub fn get_chc(&self, dna_id: &DnaId) -> Option<ChcImpl> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "chc")] {
                crate::conductor::chc::build_chc(self.config.chc_url.as_ref().map(|u| u.as_ref()), self.keystore().clone(), dna_id)
            } else {
                None
            }
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) async fn chc_sync(
        self: Arc<Self>,
        dna_id: DnaId,
        enable_app: Option<InstalledAppId>,
    ) -> ConductorApiResult<()> {
        if let Some(chc) = self.get_chc(&dna_id) {
            let db =
                self.get_or_create_authored_db(dna_id.dna_hash(), dna_id.agent_pubkey().clone())?;
            let top_hash = db
                .read_async(move |txn| SourceChainResult::Ok(chain_head_db(txn)?.map(|h| h.action)))
                .await?;
            let records = chc.get_record_data(top_hash).await?;

            self.clone()
                .graft_records_onto_source_chain(dna_id, true, records)
                .await?;
            if let Some(app_id) = enable_app {
                self.enable_app(app_id).await?;
            }
        }
        Ok(())
    }
}
