pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use fallible_iterator::FallibleIterator;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::GuestOutput;

pub struct CallIterator<R: RibosomeT, I: Invocation> {
    workspace: UnsafeInvokeZomeWorkspace,
    ribosome: R,
    invocation: I,
    remaining_zomes: Vec<ZomeName>,
    remaining_components: FnComponents,
}

impl<R: RibosomeT, I: Invocation> CallIterator<R, I> {
    pub fn new(workspace: UnsafeInvokeZomeWorkspace, ribosome: R, invocation: I) -> Self {
        Self {
            workspace,
            remaining_zomes: ribosome.zomes_to_invoke(invocation.zomes()),
            ribosome,
            remaining_components: invocation.fn_components(),
            invocation,
        }
    }
}

impl<I: Invocation + 'static> FallibleIterator for CallIterator<WasmRibosome, I> {
    type Item = GuestOutput;
    type Error = RibosomeError;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let timeout = crate::start_hard_timeout!();
        let next = Ok(match self.remaining_zomes.first() {
            // there are no zomes left, we are finished
            None => None,
            Some(zome_name) => {
                match self.remaining_components.next() {
                    Some(to_call) => {
                        match self.ribosome.maybe_call(
                            self.workspace.clone(),
                            &self.invocation,
                            zome_name,
                            to_call,
                        )? {
                            Some(result) => Some(result),
                            None => self.next()?,
                        }
                    }
                    // there are no more callbacks to call in this zome
                    // reset fn components and move to the next zome
                    None => {
                        self.remaining_components = self.invocation.fn_components();
                        self.remaining_zomes.remove(0);
                        self.next()?
                    }
                }
            }
        });
        // the total should add trivial overhead vs the inner calls
        crate::end_hard_timeout!(timeout, crate::perf::MULTI_WASM_CALL);
        next
    }
}
