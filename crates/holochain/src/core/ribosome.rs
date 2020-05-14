//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [WasmRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact
pub mod error;
pub mod guest_callback;
pub mod host_fn;
pub mod wasm_ribosome;

use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::guest_callback::CallIterator;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
use crate::fixt::ZomeNameFixturator;
use error::RibosomeResult;
use fixt::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::cell::CellId;
use holochain_types::dna::DnaFile;
use holochain_types::shims::*;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::GuestOutput;
use holochain_zome_types::HostInput;
use mockall::automock;
use std::iter::Iterator;

#[derive(Clone)]
pub struct HostContext {
    pub zome_name: ZomeName,
    allow_side_effects: AllowSideEffects,
    workspace: UnsafeInvokeZomeWorkspace,
}

fixturator!(
    HostContext,
    {
        HostContext {
            zome_name: ZomeNameFixturator::new(Empty).next().unwrap(),
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new(Empty)
                .next()
                .unwrap(),
            allow_side_effects: AllowSideEffectsFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        HostContext {
            zome_name: ZomeNameFixturator::new(Unpredictable).next().unwrap(),
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            allow_side_effects: AllowSideEffectsFixturator::new(Unpredictable)
                .next()
                .unwrap(),
        }
    },
    {
        let host_context = HostContext {
            zome_name: ZomeNameFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            allow_side_effects: AllowSideEffectsFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        host_context
    }
);

impl HostContext {
    pub fn zome_name(&self) -> ZomeName {
        self.zome_name.clone()
    }
    pub fn allow_side_effects(&self) -> AllowSideEffects {
        self.allow_side_effects
    }
}

#[derive(Clone, Copy, EnumIter)]
pub enum AllowSideEffects {
    Yes,
    No,
}
enum_fixturator!(AllowSideEffects, AllowSideEffects::No);

#[derive(Debug)]
pub struct FnComponents(Vec<String>);

/// iterating over FnComponents isn't as simple as returning the inner Vec iterator
/// we return the fully joined vector in specificity order
/// specificity is defined as consisting of more components
/// e.g. FnComponents(Vec("foo", "bar", "baz")) would return:
/// - Some("foo_bar_baz")
/// - Some("foo_bar")
/// - Some("foo")
/// - None
impl Iterator for FnComponents {
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

impl From<Vec<String>> for FnComponents {
    fn from(vs: Vec<String>) -> Self {
        Self(vs)
    }
}

pub trait Invocation: Clone // + TryInto<HostInput, Error=SerializedBytesError>
{
    fn allow_side_effects(&self) -> AllowSideEffects;
    fn zome_names(&self) -> Vec<ZomeName>;
    fn fn_components(&self) -> FnComponents;
    /// the serialized input from the host for the wasm call
    /// this is intentionally NOT a reference to self because HostInput may be huge we want to be
    /// careful about cloning invocations
    fn host_input(self) -> Result<HostInput, SerializedBytesError>;
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace;
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    #[serde(skip)]
    pub workspace: UnsafeInvokeZomeWorkspace,
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapToken,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: HostInput,
    /// the provenance of the call
    pub provenance: AgentPubKey,
    /// the hash of the top header at the time of call
    pub as_at: HeaderHash,
}

impl Invocation for ZomeInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        vec![self.zome_name.to_owned()]
    }
    fn fn_components(&self) -> FnComponents {
        vec![self.fn_name.to_owned()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(self.payload)
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

/// Response to a zome invocation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(GuestOutput),
}

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [WasmRibosome]
#[automock]
pub trait RibosomeT: Sized {
    fn dna_file(&self) -> &DnaFile;

    /// @todo list out all the available callbacks and maybe cache them somewhere
    fn list_callbacks(&self) {
        unimplemented!()
        // pseudocode
        // self.instance().exports().filter(|e| e.is_callback())
    }

    /// @todo list out all the available zome functions and maybe cache them somewhere
    fn list_zome_fns(&self) {
        unimplemented!()
        // pseudocode
        // self.instance().exports().filter(|e| !e.is_callback())
    }

    fn run_init(&self, invocation: InitInvocation) -> RibosomeResult<InitResult>;

    fn run_migrate_agent(
        &self,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult>;

    fn run_validation_package(
        &self,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult>;

    fn run_post_commit(&self, invocation: PostCommitInvocation)
        -> RibosomeResult<PostCommitResult>;

    /// Helper function for running a validation callback. Just calls
    /// [`run_callback`][] under the hood.
    /// [`run_callback`]: #method.run_callback
    fn run_validate(&self, invocation: ValidateInvocation) -> RibosomeResult<ValidateResult>;

    fn call_iterator<R: 'static + RibosomeT, I: 'static + Invocation>(
        &self,
        ribosome: R,
        invocation: I,
    ) -> CallIterator<R, I>;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        self,
        // TODO: ConductorHandle
        invocation: ZomeInvocation,
    ) -> RibosomeResult<ZomeInvocationResponse>;
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomeInvocation;
    use crate::core::ribosome::ZomeInvocationResponse;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use core::time::Duration;
    use holo_hash::holo_hash_core::HeaderHash;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::test_utils::fake_header_hash;
    use holochain_types::test_utils::{fake_agent_pubkey_1, fake_cap_token, fake_cell_id};
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::commit::CommitEntryResult;
    use holochain_zome_types::zome::ZomeName;
    use holochain_zome_types::*;
    use test_wasm_common::TestString;

    pub fn zome_invocation_from_names(
        zome_name: ZomeName,
        fn_name: &str,
        payload: SerializedBytes,
    ) -> ZomeInvocation {
        ZomeInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Empty)
                .next()
                .unwrap(),
            zome_name,
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_cap_token(),
            payload: HostInput::new(payload),
            provenance: fake_agent_pubkey_1(),
            as_at: fake_header_hash("fake"),
        }
    }

    pub fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[macro_export]
    macro_rules! call_test_ribosome {
        ( $test_wasm:expr, $fn_name:literal, $input:expr ) => {
            tokio::task::spawn(async move {
                // ensure type of test wasm
                use crate::core::ribosome::RibosomeT;
                use std::convert::TryInto;
                let ribosome =
                    $crate::fixt::WasmRibosomeFixturator::new($crate::fixt::curve::Zomes(vec![
                        $test_wasm.into(),
                    ]))
                    .next()
                    .unwrap();

                let timeout = $crate::start_hard_timeout!();

                let invocation = $crate::core::ribosome::wasm_test::zome_invocation_from_names(
                    $test_wasm.into(),
                    $fn_name,
                    $input.try_into().unwrap(),
                );
                let zome_invocation_response = ribosome.call_zome_function(invocation).unwrap();

                // instance building off a warm module should be the slowest part of a wasm test
                // so if each instance (including inner callbacks) takes ~1ms this gives us
                // headroom on 4 call(back)s
                $crate::end_hard_timeout!(timeout, 5_000_000);

                let output = match zome_invocation_response {
                    crate::core::ribosome::ZomeInvocationResponse::ZomeApiFn(guest_output) => {
                        guest_output.into_inner().try_into().unwrap()
                    }
                };
                // this is convenient for now as we flesh out the zome i/o behaviour
                // maybe in the future this will be too noisy and we might want to remove it...
                dbg!(&output);
                output
            })
            .await
            .unwrap();
        };
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn invoke_foo_test() {
        let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let invocation = zome_invocation_from_names(
            TestWasm::Foo.into(),
            "foo",
            SerializedBytes::try_from(()).unwrap(),
        );

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(GuestOutput::new(
                TestString::from(String::from("foo")).try_into().unwrap()
            )),
            ribosome.call_zome_function(invocation).unwrap()
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn pass_validate_test() {
        assert_eq!(
            CommitEntryResult::Success(HeaderHash::new(vec![0xdb; 36])),
            call_test_ribosome!(TestWasm::Validate, "always_validates", ()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn fail_validate_test() {
        assert_eq!(
            CommitEntryResult::Fail("Invalid(\"NeverValidates never validates\")".to_string()),
            call_test_ribosome!(TestWasm::Validate, "never_validates", ()),
        );
    }
}
