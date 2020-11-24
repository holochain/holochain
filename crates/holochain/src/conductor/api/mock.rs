#![allow(missing_docs)]
#![allow(clippy::ptr_arg)]

use super::CellConductorApiT;
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::entry_def_store::EntryDefBufferKey;
use crate::conductor::interface::SignalBroadcaster;
use crate::core::workflow::ZomeCallInvocationResult;
use crate::nucleus::dna::zome::Zome;
use crate::nucleus::dna::DnaFile;
use crate::nucleus::ribosome::ZomeCallInvocation;
use async_trait::async_trait;
use holo_hash::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_types::autonomic::AutonomicCue;
use holochain_types::cell::CellId;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::zome::ZomeName;
use mockall::mock;

// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
// TODO: try automock again
mock! {

    pub CellConductorApi {
        fn cell_id(&self) -> &CellId;
        fn sync_call_zome(
            &self,
            cell_id: &CellId,
            invocation: ZomeCallInvocation,
        ) -> ConductorApiResult<ZomeCallInvocationResult>;

        fn sync_autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

        fn sync_dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;

        fn mock_keystore(&self) -> &KeystoreSender;
        fn mock_signal_broadcaster(&self) -> SignalBroadcaster;
        fn sync_get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile>;
        fn sync_get_this_dna(&self) -> ConductorApiResult<DnaFile>;
        fn sync_get_zome(&self, zome_name: &ZomeName) -> ConductorApiResult<Zome>;
        fn sync_get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;
        fn into_call_zome_handle(self) -> super::CellConductorReadHandle;
    }

    trait Clone {
        fn clone(&self) -> Self;
    }
}

#[async_trait]
impl CellConductorApiT for MockCellConductorApi {
    fn cell_id(&self) -> &CellId {
        self.cell_id()
    }

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

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.mock_signal_broadcaster()
    }

    async fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile> {
        self.sync_get_dna(dna_hash)
    }

    async fn get_this_dna(&self) -> ConductorApiResult<DnaFile> {
        self.sync_get_this_dna()
    }

    async fn get_zome(&self, zome_name: &ZomeName) -> ConductorApiResult<Zome> {
        self.sync_get_zome(zome_name)
    }

    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.sync_get_entry_def(key)
    }

    fn into_call_zome_handle(self) -> super::CellConductorReadHandle {
        self.into_call_zome_handle()
    }
}
