pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod validation_package;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CallbackGuestOutput;

use crate::core::ribosome::guest_callback::validate::ValidateInvocation;

pub enum AllowSideEffects {
    Yes,
    No,
}

pub struct CallbackFnComponents(Vec<String>);

pub enum CallbackInvocation<'a> {
    Validate(ValidateInvocation<'a>),
    Init(InitInvocation<'a>),
}

pub struct CallbackIterator<R: RibosomeT> {
    ribosome: std::marker::PhantomData<R>,
}

impl Iterator for CallbackIterator<WasmRibosome<'_>> {
    type Item = CallbackGuestOutput;
    fn next(&mut self) -> Option<Self::Item> {}
}
