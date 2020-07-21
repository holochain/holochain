#![allow(missing_docs)]
#![allow(clippy::ptr_arg)]

use super::CellConductorApiT;
use crate::conductor::api::error::ConductorApiResult;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::ZomeCallInvocationResult;
use async_trait::async_trait;
use holo_hash_ext::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_types::dna::DnaFile;
use holochain_types::{autonomic::AutonomicCue, cell::CellId};
use mockall::mock;

// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
mock! {

    pub CellConductorApi {

        fn sync_call_zome(
            &self,
            cell_id: &CellId,
            invocation: ZomeCallInvocation,
        ) -> ConductorApiResult<ZomeCallInvocationResult>;

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
    async fn call_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        self.sync_call_zome(cell_id, invocation)
    }

    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String> {
        self.sync_dpki_request(method, args)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        self.sync_autonomic_cue(cue)
    }

    fn keystore(&self) -> &KeystoreSender {
        self.mock_keystore()
    }
    async fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile> {
        self.sync_get_dna(dna_hash)
    }
}
