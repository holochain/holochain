pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use fallible_iterator::FallibleIterator;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::GuestOutput;

pub struct CallIterator<R: RibosomeT, I: Invocation> {
    ribosome: R,
    invocation: I,
    remaining_zomes: Vec<ZomeName>,
    remaining_components: FnComponents,
}

impl<R: RibosomeT, I: Invocation> CallIterator<R, I> {
    pub fn new(ribosome: R, invocation: I) -> Self {
        Self {
            ribosome,
            remaining_zomes: invocation.zome_names(),
            remaining_components: invocation.fn_components(),
            invocation,
        }
    }
}

impl<I: Invocation> FallibleIterator for CallIterator<WasmRibosome, I> {
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
                        let host_context = HostContext {
                            zome_name: zome_name.clone(),
                            allow_side_effects: self.invocation.allow_side_effects(),
                            workspace: self.invocation.workspace(),
                        };
                        let module_timeout = crate::start_hard_timeout!();
                        let module = self.ribosome.module(host_context.clone())?;
                        crate::end_hard_timeout!(
                            module_timeout,
                            crate::perf::WASM_MODULE_CACHE_HIT
                        );

                        if module.info().exports.contains_key(&to_call) {
                            // there is a callback to_call and it is implemented in the wasm
                            let mut instance = self.ribosome.instance(host_context)?;

                            let call_timeout = crate::start_hard_timeout!();
                            let result: Self::Item = holochain_wasmer_host::guest::call(
                                &mut instance,
                                &to_call,
                                // be aware of this clone!
                                // the whole invocation is cloned!
                                // @todo - is this a problem for large payloads like entries?
                                self.invocation.clone().host_input()?,
                            )?;
                            crate::end_hard_timeout!(call_timeout, crate::perf::MULTI_WASM_CALL);

                            Some(result)
                        } else {
                            // the func doesn't exist
                            // the callback is not implemented
                            // skip this attempt
                            self.next()?
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
