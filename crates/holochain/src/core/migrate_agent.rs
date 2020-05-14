//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::{wasm_ribosome::WasmRibosome, RibosomeT};
use holochain_types::dna::DnaFile;

/// build a ribosome from a dna and build a validation package
pub async fn run_migrate_agent(
    dna_file: DnaFile,
    invocation: MigrateAgentInvocation,
) -> RibosomeResult<MigrateAgentResult> {
    let ribosome = WasmRibosome::new(dna_file);
    ribosome.run_migrate_agent(invocation)
}
