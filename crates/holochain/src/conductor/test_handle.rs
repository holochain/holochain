//! A wrapper around ConductorHandle with more convenient methods

use crate::{conductor::handle::ConductorHandle, core::ribosome::ZomeCallInvocation};
use hdk3::prelude::*;
use holochain_types::app::{InstalledAppId, InstalledCell};
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaFile;
use unwrap_to::unwrap_to;

/// A wrapper around ConductorHandle with more convenient methods
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct TestConductorHandle(ConductorHandle);

impl TestConductorHandle {
    /// Call a zome function with automatic de/serialization of input and output
    pub async fn call_zome<I, O, F, E>(
        &self,
        cell_id: &CellId,
        zome: &Zome,
        fn_name: F,
        cap: Option<CapSecret>,
        provenance: Option<AgentPubKey>,
        payload: I,
    ) -> O
    where
        E: std::fmt::Debug,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        let payload = ExternInput::new(payload.try_into().expect("Couldn't serialize payload"));
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        let invocation = ZomeCallInvocation {
            cell_id: cell_id.clone(),
            zome: zome.clone(),
            fn_name: fn_name.into(),
            cap,
            provenance,
            payload,
        };
        let response = self.0.call_zome(invocation).await.unwrap().unwrap();
        unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .expect("Couldn't deserialize zome call output")
    }

    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    /// All InstalledAppIds and CellNicks are auto-generated.
    /// Returns the list of generated InstalledAppIds, in the same order as Agents passed in.
    pub async fn setup_apps(
        &self,
        app_id_prefix: &str,
        dna_files: &[DnaFile],
        agents: &[AgentPubKey],
    ) -> Vec<InstalledAppId> {
        for dna_file in dna_files {
            self.install_dna(dna_file.clone()).await.unwrap()
        }

        let installed_app_ids =
            futures::future::join_all(agents.into_iter().map(|agent| async move {
                let installed_app_id = format!("{}-TODO", app_id_prefix);
                let cells = dna_files
                    .iter()
                    .map(|d| CellId::new(d.dna_hash().clone(), agent.clone()))
                    .map(|i| (InstalledCell::new(i, "TODO".into()), None))
                    .collect();
                self.0
                    .clone()
                    .install_app(installed_app_id.clone(), cells)
                    .await
                    .unwrap();
                installed_app_id
            }))
            .await;

        self.0.clone().setup_cells().await.unwrap();

        installed_app_ids
    }

    // pub async fn install(apps: HashMap<InstalledAppId, HashMap<CellNick, HashMap<ZomeName, ZomeDef>>>)  {
    //     for (installed_app_id, dnas) in apps {
    //         for (cell_nick, zomes) in dnas {
    //             DnaFile::from_inline_zomes
    //             for (zome_name, zome_def) in zomes {

    //             }
    //         }
    //     }
    // }
}
