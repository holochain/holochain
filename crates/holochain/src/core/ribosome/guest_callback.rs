pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CallbackGuestOutput;
use holochain_zome_types::CallbackHostInput;
use crate::core::ribosome::error::RibosomeError;
use fallible_iterator::FallibleIterator;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::zome::ZomeName;
use crate::core::ribosome::host_fn::AllowSideEffects;
use crate::core::ribosome::host_fn::HostContext;

pub struct CallbackFnComponents(Vec<String>);

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

/// simple trait allows &CallbackInvocation to delegate for data efficiently
/// impl this trait on &Foo rather than Foo so we can easily avoid cloning at call time
pub trait Invocation: Into<CallbackFnComponents> + TryInto<CallbackHostInput> + Into<Vec<ZomeName>> + Into<AllowSideEffects> {}

// impl From<&CallbackInvocation<'_>> for CallbackFnComponents {
//     fn from(callback_invocation: &CallbackInvocation) -> Self {
//         match callback_invocation {
//             CallbackInvocation::Validate(invocation) => invocation.into(),
//             CallbackInvocation::Init(invocation) => invocation.into(),
//         }
//     }
// }

pub struct CallbackIterator<R: RibosomeT, I: Invocation> {
    ribosome: R,
    invocation: I,
    remaining_zomes: Vec<ZomeName>,
    remaining_components: CallbackFnComponents,
}

impl <R: RibosomeT, I: Invocation>CallbackIterator<R, I> {
    pub fn new(ribosome: R, invocation: I) -> Self {
        Self {
            ribosome,
            invocation,
            remaining_zomes: invocation.into(),
            remaining_components: invocation.into()
        }
    }
}

impl Iterator for CallbackFnComponents {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        match self.0.len() {
            0 => None,
            _ => {
                let ret = self.0.join("_");
                self.0.pop();
                Some(ret)
            }
        }
    }
}

impl <I: Invocation<Error = SerializedBytesError>>FallibleIterator for CallbackIterator<WasmRibosome, I> {
    type Item = CallbackGuestOutput;
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
                            allow_side_effects: self.invocation.into(),
                        })?;
                        match instance.resolve_func(&to_call) {
                            // there is a callback to_call and it is implemented in the wasm
                            Ok(_) => {
                                let payload: CallbackHostInput = self.invocation.try_into()?;
                                let result: Self::Item = holochain_wasmer_host::guest::call(
                                    &mut instance,
                                    &to_call,
                                    payload
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
                        self.remaining_components = self.invocation.into();
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
// ) -> RibosomeResult<Vec<Option<CallbackGuestOutput>>> {
//     let mut fn_components = invocation.components.clone();
//     let mut results: Vec<Option<CallbackGuestOutput>> = vec![];
//     loop {
//         if fn_components.len() > 0 {
//             let mut instance =
//                 self.instance(HostContext::from(&invocation), allow_side_effects)?;
//             let fn_name = fn_components.join("_");
//             match instance.resolve_func(&fn_name) {
//                 Ok(_) => {
//                     let wasm_callback_response: CallbackGuestOutput =
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
// ) -> RibosomeResult<Vec<Option<CallbackGuestOutput>>> {
//     let mut fn_components = invocation.components.clone();
//     let mut results: Vec<Option<CallbackGuestOutput>> = vec![];
//     loop {
//         if fn_components.len() > 0 {
//             let mut instance =
//                 self.instance(HostContext::from(&invocation), allow_side_effects)?;
//             let fn_name = fn_components.join("_");
//             match instance.resolve_func(&fn_name) {
//                 Ok(_) => {
//                     let wasm_callback_response: CallbackGuestOutput =
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
