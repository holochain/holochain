use std::sync::Arc;

use holochain_keystore::MetaLairClient;
use holochain_types::prelude::*;

use crate::CellRunner;

pub mod derivation_paths;

/// This magic string, when used as the installed app id, denotes that the app
/// is not actually an app, but the DPKI service! This is now a reserved app id,
/// and is used to distinguish the DPKI service from other apps.
pub const DPKI_APP_ID: &str = "DPKI";

pub type DpkiMutex = Option<Arc<tokio::sync::Mutex<dyn DpkiService>>>;

/// Interface for the DPKI service
#[async_trait::async_trait]
#[mockall::automock]
#[allow(clippy::needless_lifetimes)]
pub trait DpkiService: Send + Sync {
    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp
    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState>;

    /// Derive a new key in lair using the given index, and register it with DPKI
    async fn derive_and_register_new_key(&self) -> DpkiServiceResult<AgentPubKey>;

    /// Defines the different ways that keys can be created and destroyed:
    /// If an old key is specified, it will be destroyed
    /// If a new key is specified, it will be registered
    /// If both a new and an old key are specified, the new key will atomically replace the old key
    /// (If no keys are specified, nothing will happen)
    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<()>;

    /// The CellId which backs this service
    fn cell_id(&self) -> &CellId;
}

/// Mirrors the output type of the "key_state" zome function in dpki
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyState {
    NotFound,
    Invalidated(SignedActionHashed),
    Valid(SignedActionHashed),
}

impl KeyState {
    pub fn is_valid(&self) -> bool {
        matches!(self, KeyState::Valid(_))
    }
}

/// The errors which can be produced by DPKI
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum DpkiServiceError {
    #[error("DPKI DNA could not be called: {0}")]
    ZomeCallFailed(anyhow::Error),
    #[error(transparent)]
    Serialization(#[from] SerializedBytesError),
    #[error("Error talking to lair keystore: {0}")]
    Lair(anyhow::Error),
}
/// Alias
pub type DpkiServiceResult<T> = Result<T, DpkiServiceError>;

/// Some more helpful methods built around the methods provided by the service
#[async_trait::async_trait]
pub trait DpkiServiceExt: DpkiService {
    /// Register a newly created key with DPKI
    async fn register_key(&self, key: AgentPubKey) -> DpkiServiceResult<()> {
        self.key_mutation(None, Some(key)).await
    }

    /// Replace an old key with a new one
    async fn update_key(
        &self,
        old_key: AgentPubKey,
        new_key: AgentPubKey,
    ) -> DpkiServiceResult<()> {
        self.key_mutation(Some(old_key), Some(new_key)).await
    }

    /// Delete an existing key without replacing it with a new one.
    /// This effectively terminates the "lineage" that this key was a part of.
    async fn remove_key(&self, key: AgentPubKey) -> DpkiServiceResult<()> {
        self.key_mutation(Some(key), None).await
    }
}
impl<T> DpkiServiceExt for T where T: DpkiService + Sized {}

/// Data needed to initialize the DPKI service, if installed
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize, Debug, SerializedBytes)]
pub struct DpkiInstallation {
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

/// The built-in implementation of the DPKI service contract, which runs a DNA
pub struct DeepkeyBuiltin {
    runner: Arc<dyn CellRunner>,
    keystore: MetaLairClient,
    installation: DpkiInstallation,
}

impl DeepkeyBuiltin {
    pub fn new(
        runner: Arc<dyn CellRunner>,
        keystore: MetaLairClient,
        installation: DpkiInstallation,
    ) -> DpkiMutex {
        Some(Arc::new(tokio::sync::Mutex::new(Self {
            runner,
            keystore,
            installation,
        })))
    }
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
#[allow(clippy::needless_lifetimes)]
#[async_trait::async_trait]
impl DpkiService for DeepkeyBuiltin {
    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState> {
        let keystore = self.keystore.clone();
        let cell_id = self.installation.cell_id.clone();
        let agent_anchor = key.get_raw_32();
        let zome_name: ZomeName = "deepkey".into();
        let fn_name: FunctionName = "key_state".into();
        let payload = ExternIO::encode((agent_anchor, timestamp))?;
        let cap_secret = None;
        let provenance = cell_id.agent_pubkey().clone();
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

    async fn derive_and_register_new_key(&self) -> DpkiServiceResult<AgentPubKey> {
        let derivation_path = todo!("get path from deepkey DNA");
        let info = self
            .keystore
            .lair_client()
            .derive_seed(todo!(), None, todo!(), None, derivation_path)
            .await
            .map_err(|e| DpkiServiceError::Lair(e.into()))?;
        let agent = AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());
        Ok(agent)
    }

    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<()> {
        todo!()
    }

    fn cell_id(&self) -> &CellId {
        &self.installation.cell_id
    }
}

/// Create a minimal usable mock of DPKI
#[cfg(feature = "fuzzing")]
pub fn mock_dpki() -> MockDpkiService {
    use arbitrary::Arbitrary;
    use futures::FutureExt;

    let mut dpki = MockDpkiService::new();
    let mut u = unstructured_noise();
    let action = SignedActionHashed::arbitrary(&mut u).unwrap();
    dpki.expect_key_state().returning(move |_, _| {
        let action = action.clone();
        async move { Ok(KeyState::Valid(action)) }.boxed()
    });
    dpki.expect_cell_id().return_const(fake_cell_id(0));
    dpki
}
