use super::*;
use crate::core::ribosome::{
    error::RibosomeError,
    guest_callback::init::InitHostAccess,
    guest_callback::init::InitInvocation,
    guest_callback::validate::ValidateHostAccess,
    guest_callback::validate::ValidateInvocation,
    guest_callback::validate::ValidateResult,
    guest_callback::CallIterator,
    guest_callback::{
        entry_defs::{EntryDefsHostAccess, EntryDefsInvocation, EntryDefsResult},
        init::InitResult,
        migrate_agent::{MigrateAgentHostAccess, MigrateAgentInvocation, MigrateAgentResult},
        post_commit::PostCommitHostAccess,
        post_commit::{PostCommitInvocation, PostCommitResult},
        validate_link::{ValidateLinkHostAccess, ValidateLinkInvocation, ValidateLinkResult},
        validation_package::ValidationPackageHostAccess,
        validation_package::ValidationPackageInvocation,
        validation_package::ValidationPackageResult,
    },
    HostAccess, Invocation, RibosomeT, ZomeCallHostAccess, ZomeCallInvocation, ZomesToInvoke,
};
use holochain_types::dna::DnaFile;
use holochain_zome_types::prelude::*;

impl<'f> RibosomeT for InlineDna<'f> {
    fn dna_file(&self) -> &DnaFile {
        todo!()
    }

    fn zomes_to_invoke(&self, zomes_to_invoke: ZomesToInvoke) -> Vec<ZomeName> {
        todo!()
    }

    fn zome_name_to_id(&self, zome_name: &ZomeName) -> RibosomeResult<ZomeId> {
        todo!()
    }

    /// call a function in a zome for an invocation if it exists
    /// if it does not exist then return Ok(None)
    fn maybe_call<I: Invocation>(
        &self,
        host_access: HostAccess,
        invocation: &I,
        zome_name: &ZomeName,
        to_call: &FunctionName,
    ) -> Result<Option<ExternOutput>, RibosomeError> {
        todo!()
    }

    fn call_iterator<R: RibosomeT, I: crate::core::ribosome::Invocation>(
        &self,
        access: HostAccess,
        ribosome: R,
        invocation: I,
    ) -> CallIterator<R, I> {
        todo!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        &self,
        host_access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse> {
        todo!()
    }

    fn run_validate(
        &self,
        access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        todo!()
    }

    fn run_validate_link<I: Invocation + 'static>(
        &self,
        access: ValidateLinkHostAccess,
        invocation: ValidateLinkInvocation<I>,
    ) -> RibosomeResult<ValidateLinkResult> {
        todo!()
    }

    fn run_init(
        &self,
        access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        todo!()
    }

    fn run_entry_defs(
        &self,
        access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        todo!()
    }

    fn run_migrate_agent(
        &self,
        access: MigrateAgentHostAccess,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult> {
        todo!()
    }

    fn run_validation_package(
        &self,
        access: ValidationPackageHostAccess,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult> {
        todo!()
    }

    fn run_post_commit(
        &self,
        access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<PostCommitResult> {
        todo!()
    }
}
