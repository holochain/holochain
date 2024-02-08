

use super::*;

use crate::CellRunner;
use holochain_keystore::MetaLairClient;

/// The built-in implementation of the DPKI service contract, which runs a DNA
pub struct DeepkeyBuiltin {
    runner: Arc<dyn CellRunner>,
    keystore: MetaLairClient,
    installation: DeepkeyInstallation,
}

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

impl DeepkeyBuiltin {
    pub fn new(
        runner: Arc<dyn CellRunner>,
        keystore: MetaLairClient,
        installation: DeepkeyInstallation,
    ) -> DpkiMutex {
        Arc::new(tokio::sync::Mutex::new(Self {
            runner,
            keystore,
            installation,
        }))
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

    async fn derive_and_register_new_key(
        &self,
        app_name: InstalledAppId,
        dna_hashes: Vec<DnaHash>,
    ) -> DpkiServiceResult<AgentPubKey> {
        let derivation_details: DerivationDetailsInput = {
            let cell_id = self.installation.cell_id.clone();
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
                .decode()?
        };

        let info = self
            .keystore
            .lair_client()
            .derive_seed(
                self.installation.device_seed_lair_tag.clone().into(),
                None,
                nanoid::nanoid!().into(),
                None,
                derivation_details.to_derivation_path().into_boxed_slice(),
            )
            .await
            .map_err(|e| DpkiServiceError::Lair(e.into()))?;
        let app_agent = AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());

        let dpki_agent = self.cell_id().agent_pubkey().clone();

        // This is the signature Deepkey requires
        let signature = app_agent
            .sign_raw(&self.keystore, dpki_agent.get_raw_39().into())
            .await
            .map_err(|e| DpkiServiceError::Lair(e.into()))?;

        #[cfg(test)]
        assert_eq!(
            hdk::prelude::verify_signature_raw(
                app_agent.clone(),
                signature.clone(),
                dpki_agent.get_raw_39().to_vec()
            ),
            Ok(true)
        );

        let input = CreateKeyInput {
            key_generation: KeyGeneration {
                new_key: app_agent.clone(),
                new_key_signing_of_author: signature,
            },
            app_binding: AppBindingInput {
                app_name: app_name.clone(),
                installed_app_id: app_name,
                dna_hashes,
            },
            derivation_details,
        };

        let _: (ActionHash, KeyRegistration, KeyMeta) = {
            let cell_id = self.installation.cell_id.clone();
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
                .decode()?
        };

        Ok(app_agent)
    }

    fn cell_id(&self) -> &CellId {
        &self.installation.cell_id
    }
}
