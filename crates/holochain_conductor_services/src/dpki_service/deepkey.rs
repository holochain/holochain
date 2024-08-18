use serde::de::DeserializeOwned;

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

const DEEPKEY_ZOME_NAME: &str = "deepkey_csr";

impl DeepkeyState {
    async fn call_deepkey_zome<
        I: serde::Serialize + std::fmt::Debug,
        O: std::fmt::Debug + DeserializeOwned,
    >(
        &self,
        fn_name: &str,
        input: I,
    ) -> DpkiServiceResult<O> {
        let cell_id = self.cell_id.clone();
        let provenance = cell_id.agent_pubkey().clone();
        let cap_secret = None;
        let zome_name: ZomeName = DEEPKEY_ZOME_NAME.into();
        let fn_name: FunctionName = fn_name.into();
        let payload = ExternIO::encode(input)?;
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
}

// Tests for these calls are located in the Holochain conductor package in the form of
// full integration tests.
#[async_trait::async_trait]
impl DpkiState for DeepkeyState {
    async fn next_derivation_details(
        &self,
        agent_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<DerivationDetails> {
        let payload = agent_key.map(|agent_key| {
            serde_bytes::ByteArray::<32>::new(agent_key.get_raw_32().try_into().unwrap())
        });
        self.call_deepkey_zome("next_derivation_details", payload)
            .await
    }

    async fn register_key(
        &self,
        input: CreateKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration, KeyMeta)> {
        self.call_deepkey_zome("create_key", input).await
    }

    async fn query_key_meta(&self, agent_key: AgentPubKey) -> DpkiServiceResult<KeyMeta> {
        let payload = agent_key.get_raw_32();
        self.call_deepkey_zome("query_key_meta_for_key", payload)
            .await
    }

    async fn revoke_key(
        &self,
        input: RevokeKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration)> {
        self.call_deepkey_zome("revoke_key", input).await
    }

    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState> {
        let agent_anchor = key.get_raw_32();
        let payload = (agent_anchor, timestamp);
        self.call_deepkey_zome("key_state", payload).await
    }

    async fn is_same_agent(
        &self,
        key_1: AgentPubKey,
        key_2: AgentPubKey,
    ) -> DpkiServiceResult<bool> {
        self.call_deepkey_zome("same_lineage", (key_1.get_raw_32(), key_2.get_raw_32()))
            .await
    }
}
