#![allow(missing_docs)]
#![allow(clippy::ptr_arg)]

use super::CellConductorApiT;
use crate::conductor::api::error::ConductorApiResult;
use crate::core::workflow::{ZomeInvocationExternal, ZomeInvocationResult};
use async_trait::async_trait;
use holo_hash::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_types::dna::DnaFile;
use holochain_types::{autonomic::AutonomicCue, cell::CellId, prelude::Todo};
use mockall::mock;

// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
mock! {

    pub CellConductorApi {

        fn sync_invoke_zome(
            &self,
            cell_id: &CellId,
            invocation: ZomeInvocationExternal,
        ) -> ConductorApiResult<ZomeInvocationResult>;

        fn sync_network_send(&self, message: Todo) -> ConductorApiResult<()>;

        fn sync_network_request(
            &self,
            _message: Todo,
        ) -> ConductorApiResult<Todo>;

        fn sync_autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

        fn sync_dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;

        fn mock_keystore(&self) -> &KeystoreSender;
        fn sync_get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile>;
    }

    trait Clone {
        fn clone(&self) -> Self;
    }
}

#[async_trait]
impl CellConductorApiT for MockCellConductorApi {
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocationExternal,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        self.sync_invoke_zome(cell_id, invocation)
    }

    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String> {
        self.sync_dpki_request(method, args)
    }

    async fn network_send(&self, message: Todo) -> ConductorApiResult<()> {
        self.sync_network_send(message)
    }

    async fn network_request(&self, message: Todo) -> ConductorApiResult<Todo> {
        self.sync_network_request(message)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        self.sync_autonomic_cue(cue)
    }

    fn keystore(&self) -> &KeystoreSender {
        self.mock_keystore()
    }
    async fn get_dna<'a>(&'a self, dna_hash: &'a DnaHash) -> Option<DnaFile> {
        self.sync_get_dna(dna_hash)
    }
}
