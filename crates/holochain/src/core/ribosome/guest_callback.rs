pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::GuestOutput;
use crate::core::ribosome::error::RibosomeError;
use fallible_iterator::FallibleIterator;
use holochain_zome_types::zome::ZomeName;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::FnComponents;

// pub enum CallbackInvocation<'a> {
//     Validate(ValidateInvocation<'a>),
//     Init(InitInvocation<'a>),
// }
//
// impl <'a>From<InitInvocation<'a>> for CallbackInvocation<'a> {
//     fn from(init_invocation: InitInvocation<'a>) -> Self {
//         Self::Init(init_invocation)
//     }
// }

// impl From<&CallbackInvocation<'_>> for FnComponents {
//     fn from(callback_invocation: &CallbackInvocation) -> Self {
//         match callback_invocation {
//             CallbackInvocation::Validate(invocation) => invocation.into(),
//             CallbackInvocation::Init(invocation) => invocation.into(),
//         }
//     }
// }

pub struct CallIterator<R: RibosomeT, I: Invocation> {
    ribosome: R,
    invocation: I,
    remaining_zomes: Vec<ZomeName>,
    remaining_components: FnComponents,
}

impl <R: RibosomeT, I: Invocation>CallIterator<R, I> {
    pub fn new(ribosome: R, invocation: I) -> Self {
        Self {
            ribosome,
            remaining_zomes: invocation.zome_names(),
            remaining_components: invocation.fn_components(),
            invocation,
        }
    }
}

impl <I: Invocation>FallibleIterator for CallIterator<WasmRibosome, I> {
    type Item = GuestOutput;
    type Error = RibosomeError;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Ok(match self.remaining_zomes.first() {
            // there are no zomes left, we are finished
            None => None,
            Some(zome_name) => {
                match self.remaining_components.next() {
                    Some(to_call) => {
                        let mut instance = self.ribosome.instance(HostContext {
                            zome_name: zome_name.clone(),
                            allow_side_effects: self.invocation.allow_side_effects(),
                        })?;
                        match instance.resolve_func(&to_call) {
                            // there is a callback to_call and it is implemented in the wasm
                            Ok(_) => {
                                let result: Self::Item = holochain_wasmer_host::guest::call(
                                    &mut instance,
                                    &to_call,
                                    // be aware of this clone!
                                    // the whole invocation is cloned!
                                    // @todo - is this a problem for large payloads like entries?
                                    self.invocation.clone().host_input()?
                                )?;
                                Some(result)
                            },
                            // the func doesn't exist
                            // the callback is not implemented
                            // skip this attempt
                            Err(_) => self.next()?,
                        }
                    },
                    // there are no more callbacks to call in this zome
                    // reset fn components and move to the next zome
                    None => {
                        self.remaining_components = self.invocation.fn_components();
                        self.remaining_zomes.remove(0);
                        self.next()?
                    },
                }
            }
        })
    }
}

// fn run_callback(
//     &self,
//     invocation: CallbackInvocation,
//     allow_side_effects: bool,
// ) -> RibosomeResult<Vec<Option<GuestOutput>>> {
//     let mut fn_components = invocation.components.clone();
//     let mut results: Vec<Option<GuestOutput>> = vec![];
//     loop {
//         if fn_components.len() > 0 {
//             let mut instance =
//                 self.instance(HostContext::from(&invocation), allow_side_effects)?;
//             let fn_name = fn_components.join("_");
//             match instance.resolve_func(&fn_name) {
//                 Ok(_) => {
//                     let wasm_callback_response: GuestOutput =
//                         holochain_wasmer_host::guest::call(
//                             &mut instance,
//                             &fn_name,
//                             invocation.payload.clone(),
//                         )?;
//                     results.push(Some(wasm_callback_response));
//                 }
//                 Err(_) => results.push(None),
//             }
//             fn_components.pop();
//         } else {
//             break;
//         }
//     }
//
//     // reverse the vector so that most specific results are first
//     Ok(results.into_iter().rev().collect())
// }

// fn run_callback(
//     &self,
//     invocation: CallbackInvocation,
//     allow_side_effects: bool,
// ) -> RibosomeResult<Vec<Option<GuestOutput>>> {
//     let mut fn_components = invocation.components.clone();
//     let mut results: Vec<Option<GuestOutput>> = vec![];
//     loop {
//         if fn_components.len() > 0 {
//             let mut instance =
//                 self.instance(HostContext::from(&invocation), allow_side_effects)?;
//             let fn_name = fn_components.join("_");
//             match instance.resolve_func(&fn_name) {
//                 Ok(_) => {
//                     let wasm_callback_response: GuestOutput =
//                         holochain_wasmer_host::guest::call(
//                             &mut instance,
//                             &fn_name,
//                             invocation.payload.clone(),
//                         )?;
//                     results.push(Some(wasm_callback_response));
//                 }
//                 Err(_) => results.push(None),
//             }
//             fn_components.pop();
//         } else {
//             break;
//         }
//     }
//
//     // reverse the vector so that most specific results are first
//     Ok(results.into_iter().rev().collect())
// }
