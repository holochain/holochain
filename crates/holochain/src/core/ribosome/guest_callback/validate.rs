use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
use crate::fixt::EntryFixturator;
use crate::fixt::ZomeNameFixturator;
use fixt::prelude::*;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::sync::Arc;

#[derive(Clone)]
pub struct ValidateInvocation {
    // @todo ValidateWorkspace?
    pub workspace: UnsafeInvokeZomeWorkspace,
    pub zome_name: ZomeName,
    // Arc here as entry may be very large
    // don't want to clone the Entry just to validate it
    // we can SerializedBytes off an Entry reference
    // lifetimes on invocations are a pain
    pub entry: Arc<Entry>,
}

fixturator!(
    ValidateInvocation,
    {
        let validate_invocation = ValidateInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            zome_name: ZomeNameFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            entry: Arc::new(
                EntryFixturator::new_indexed(Empty, self.0.index)
                    .next()
                    .unwrap(),
            ),
        };
        self.0.index = self.0.index + 1;
        validate_invocation
    },
    {
        let validate_invocation = ValidateInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(
                Unpredictable,
                self.0.index,
            )
            .next()
            .unwrap(),
            zome_name: ZomeNameFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            entry: Arc::new(
                EntryFixturator::new_indexed(Unpredictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
        };
        self.0.index = self.0.index + 1;
        validate_invocation
    },
    {
        let validate_invocation = ValidateInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            zome_name: ZomeNameFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            entry: Arc::new(
                EntryFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            ),
        };
        self.0.index = self.0.index + 1;
        validate_invocation
    }
);

impl Invocation for ValidateInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::No
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        // entries are specific to zomes so only validate in the zome the entry is defined in
        // note that here it is possible there is a zome/entry mismatch
        // we rely on the invocation to be built correctly
        vec![self.zome_name.clone()]
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "validate".into(),
            match *self.entry {
                Entry::Agent(_) => "agent",
                Entry::App(_) => "entry",
                Entry::CapTokenClaim(_) => "cap_token_claim",
                Entry::CapTokenGrant(_) => "cap_token_grant",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&*self.entry).try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

impl TryFrom<ValidateInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(validate_invocation: ValidateInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&*validate_invocation.entry).try_into()?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<Vec<ValidateCallbackResult>> for ValidateResult {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCallbackResult::Invalid(i) => Self::Invalid(i),
                // return unresolved dependencies if it's otherwise valid
                ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud.into_iter().map(|h| h.into()).collect()),
                },
                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {

    use super::ValidateInvocationFixturator;
    use super::ValidateResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome.run_validate(validate_invocation).unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_success() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateValid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateValid.into();

        let result = ribosome.run_validate(validate_invocation).unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_fail() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateInvalid.into();

        let result = ribosome.run_validate(validate_invocation).unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()),);
    }
}
