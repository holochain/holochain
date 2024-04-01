use super::*;

use crate::CellRunner;

/// Data needed to initialize the Deepkey service, if installed.
/// FIXME: this assumes that DPKI will be implemented by a cell, which may not
/// be the case in general. To generalize is currently out of scope.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize, Debug, SerializedBytes)]
pub struct DeepkeyInstallation {
    /// The initial cell ID used by the DPKI service.
    ///
    /// The AgentPubKey of this cell was generated from the DPKI "device seed".
    /// Upon installation, the first derivation of the seed is used.
    /// Agent key updates use subsequent derivations.
    pub cell_id: CellId,

    /// The lair tag used to refer to the "device seed" which was used to generate
    /// the AgentPubKey for the DPKI cell
    pub device_seed_lair_tag: String,
}

pub struct DeepkeyState {
    pub(crate) runner: Arc<dyn CellRunner>,
    pub(crate) cell_id: CellId,
}

#[async_trait::async_trait]
impl DpkiState for DeepkeyState {
    // fn uuid(&self) -> [u8; 32] {
    //     self.cell_id.dna_hash().get_raw_32().try_into().unwrap()
    // }

    // fn cell_id(&self) -> Option<CellId> {
    //     Some(self.cell_id.clone())
    // }

    async fn next_derivation_details(
        &self,
        app_name: InstalledAppId,
    ) -> DpkiServiceResult<DerivationDetailsInput> {
        let cell_id = self.cell_id.clone();
        let provenance = cell_id.agent_pubkey().clone();
        let zome_name: ZomeName = "deepkey_csr".into();
        let fn_name: FunctionName = "next_derivation_details".into();
        let payload = ExternIO::encode(app_name.clone())?;
        let cap_secret = None;
        self.runner
            .call_zome(
                &provenance,
                cap_secret,
                cell_id,
                zome_name,
                fn_name,
                payload,
            )
            .await
            .map_err(DpkiServiceError::ZomeCallFailed)?
            .decode()
            .map_err(Into::into)
    }

    async fn register_key(
        &self,
        input: CreateKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration, KeyMeta)> {
        let cell_id = self.cell_id.clone();
        let provenance = cell_id.agent_pubkey().clone();
        let zome_name: ZomeName = "deepkey_csr".into();
        let fn_name: FunctionName = "create_key".into();
        let payload = ExternIO::encode(input)?;
        let cap_secret = None;
        self.runner
            .call_zome(
                &provenance,
                cap_secret,
                cell_id,
                zome_name,
                fn_name,
                payload,
            )
            .await
            .map_err(DpkiServiceError::ZomeCallFailed)?
            .decode()
            .map_err(Into::into)
    }

    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState> {
        let cell_id = self.cell_id.clone();
        let provenance = cell_id.agent_pubkey().clone();
        let agent_anchor = key.get_raw_32();
        let zome_name: ZomeName = "deepkey_csr".into();
        let fn_name: FunctionName = "key_state".into();
        let payload = ExternIO::encode((agent_anchor, timestamp))?;
        let cap_secret = None;
        let response = self
            .runner
            .call_zome(
                &provenance,
                cap_secret,
                cell_id,
                zome_name,
                fn_name,
                payload,
            )
            .await
            .map_err(DpkiServiceError::ZomeCallFailed)?;
        let state: KeyState = response.decode()?;
        Ok(state)
    }
}
