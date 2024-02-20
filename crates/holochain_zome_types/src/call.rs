use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_wasmer_common::WasmError;

/// Identifier for an App Role, a foundational concept in the App manifest.
pub type RoleName = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CallTargetCell {
    OtherCell(CellId),
    OtherRole(RoleName),
    Local,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CallTarget {
    NetworkAgent(AgentPubKey),
    ConductorCell(CallTargetCell),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Call {
    pub target: CallTarget,
    pub zome_name: ZomeName,
    pub fn_name: FunctionName,
    pub cap_secret: Option<CapSecret>,
    pub payload: ExternIO,
}

impl Call {
    pub fn new(
        target: CallTarget,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
    ) -> Self {
        Self {
            target,
            zome_name,
            fn_name,
            cap_secret,
            payload,
        }
    }

    pub fn target(&self) -> &CallTarget {
        &self.target
    }

    pub fn zome_name(&self) -> &ZomeName {
        &self.zome_name
    }

    pub fn fn_name(&self) -> &FunctionName {
        &self.fn_name
    }

    pub fn cap_secret(&self) -> Option<&CapSecret> {
        self.cap_secret.as_ref()
    }

    pub fn payload(&self) -> &ExternIO {
        &self.payload
    }
}

#[allow(missing_docs)]
pub trait CallbackResult: Sized {
    /// if a callback result is definitive we should halt any further iterations over remaining
    /// calls e.g. over sparse names or subsequent zomes
    /// typically a clear failure is definitive but success and missing dependencies are not
    /// in the case of success or missing deps, a subsequent callback could give us a definitive
    /// answer like a fail, and we don't want to over-optimise wasm calls and miss a clear failure
    fn is_definitive(&self) -> bool;
    /// when a WasmError is returned from a callback (e.g. via `?` operator) it might mean either:
    ///
    /// - There was an error that prevented the callback from coming to a CallbackResult (e.g. failing to connect to database)
    /// - There was an error that should be interpreted as a CallbackResult::Fail (e.g. data failed to deserialize)
    ///
    /// Typically this can be split as host/wasm errors are the former, and serialization/guest errors the latter.
    /// This function allows each CallbackResult to explicitly map itself.
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError>;
}
