pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CallbackGuestOutput;
use holochain_zome_types::CallbackHostInput;
use std::sync::Arc;
use crate::core::ribosome::error::RibosomeError;
use fallible_iterator::FallibleIterator;
use holochain_serialized_bytes::prelude::*;
use holochain_types::nucleus::ZomeName;

pub enum AllowSideEffects {
    Yes,
    No,
}

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
pub trait Invocation: Into<CallbackFnComponents> + TryInto<CallbackHostInput> + Into<ZomeName> {}

// impl From<&CallbackInvocation<'_>> for CallbackFnComponents {
//     fn from(callback_invocation: &CallbackInvocation) -> Self {
//         match callback_invocation {
//             CallbackInvocation::Validate(invocation) => invocation.into(),
//             CallbackInvocation::Init(invocation) => invocation.into(),
//         }
//     }
// }

pub struct CallbackIterator<R: RibosomeT, I: Invocation> {
    ribosome: Arc<R>,
    invocation: I,
    remaining_components: CallbackFnComponents,
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

impl <I: Invocation<Error = SerializedBytesError>>FallibleIterator for CallbackIterator<WasmRibosome<'_>, I> {
    type Item = CallbackGuestOutput;
    type Error = RibosomeError;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Ok(match self.remaining_components.next() {
            Some(to_call) => {
                let mut instance = self.ribosome.instance((&self.invocation).into())?;
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
            }
            // there are no more callbacks to call
            None => None,
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
