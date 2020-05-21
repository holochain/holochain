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

use crate::core::ribosome::error::RibosomeError;
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
use crate::fixt::HostInputFixturator;
use crate::fixt::ZomeNameFixturator;
use error::RibosomeResult;
use fixt::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::AgentPubKeyFixturator;
use holochain_serialized_bytes::prelude::*;
use holochain_types::cell::CellId;
use holochain_types::cell::CellIdFixturator;
use holochain_types::dna::DnaFile;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::GuestOutput;
use holochain_zome_types::{capability::CapSecret, HostInput};
use mockall::automock;
use std::iter::Iterator;

#[derive(Clone)]
pub struct HostContext {
    pub zome_name: ZomeName,
    allow_side_effects: bool,
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
            allow_side_effects: BoolFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        HostContext {
            zome_name: ZomeNameFixturator::new(Unpredictable).next().unwrap(),
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            allow_side_effects: BoolFixturator::new(Unpredictable).next().unwrap(),
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
            allow_side_effects: BoolFixturator::new_indexed(Predictable, self.0.index)
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
    pub fn allow_side_effects(&self) -> bool {
        self.allow_side_effects
    }
}

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

pub enum ZomesToInvoke {
    All,
    One(ZomeName),
}

pub trait Invocation: Clone {
    /// Invocations can be externally driven (e.g. by a websockets client) or internally (e.g. from
    /// a callback triggered by the subconscious in order to allow the conscious to provide
    /// feedback. In some of these cases we allow side effects to be possible, such as committing a
    /// new entry, which will in turn trigger other callbacks, such as validation, that must be
    /// pure functions on the input data. For pure callbacks, allow_side_effects must return false.
    /// In the case that allow_side_effects is false, any call to a host function with side effects
    /// should be an unreachable!() error and halt execution. This is a panic because the happ
    /// developer must avoid use of any host function calls that produce side effects while
    /// implementing callbacks that must be pure.
    fn allow_side_effects(&self) -> bool;
    /// Some invocations call into a single zome and some call into many or all zomes.
    /// An example of an invocation that calls across all zomes is init. Init must pass for every
    /// zome in order for the Dna overall to successfully init.
    /// An example of an invocation that calls a single zome is validation of an entry, because
    /// the entry is only defined in a single zome, so it only makes sense for that exact zome to
    /// define the validation logic for that entry.
    /// In the future this may be expanded to support a subset of zomes that is larger than one.
    /// For example, we may want to trigger a callback in all zomes that implement a
    /// trait/interface, but this doesn't exist yet, so the only valid options are All or One.
    fn zomes(&self) -> ZomesToInvoke;
    /// Invocations execute in a "sparse" manner of decreasing specificity. In technical terms this
    /// means that the list of strings in FnComponents will be concatenated into a single function
    /// name to be called, then the last string will be removed and a shorter function name will
    /// be attempted and so on until all variations have been attempted.
    /// For example, if FnComponents was vec!["foo", "bar", "baz"] it would loop as "foo_bar_baz"
    /// then "foo_bar" then "foo". All of those three callbacks that are defined will be called
    /// _unless a definitive callback result is returned_.
    /// @see CallbackResult::is_definitive() in zome_types.
    /// All of the individual callback results are then folded into a single overall result value
    /// as a From implementation on the invocation results structs (e.g. zome results vs. ribosome
    /// results).
    fn fn_components(&self) -> FnComponents;
    /// the serialized input from the host for the wasm call
    /// this is intentionally NOT a reference to self because HostInput may be huge we want to be
    /// careful about cloning invocations
    fn host_input(self) -> Result<HostInput, SerializedBytesError>;
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapSecret,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: HostInput,
    /// the provenance of the call
    pub provenance: AgentPubKey,
}

fixturator!(
    ZomeInvocation,
    {
        ZomeInvocation {
            cell_id: CellIdFixturator::new(Empty).next().unwrap(),
            zome_name: ZomeNameFixturator::new(Empty).next().unwrap(),
            cap: todo!("capability arg"),
            fn_name: StringFixturator::new(Empty).next().unwrap(),
            payload: HostInputFixturator::new(Empty).next().unwrap(),
            provenance: AgentPubKeyFixturator::new(Empty).next().unwrap(),
        }
    },
    {
        ZomeInvocation {
            cell_id: CellIdFixturator::new(Unpredictable).next().unwrap(),
            zome_name: ZomeNameFixturator::new(Unpredictable).next().unwrap(),
            cap: todo!("capability arg"),
            fn_name: StringFixturator::new(Unpredictable).next().unwrap(),
            payload: HostInputFixturator::new(Unpredictable).next().unwrap(),
            provenance: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
        }
    },
    {
        let ret = ZomeInvocation {
            cell_id: CellIdFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            zome_name: ZomeNameFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            cap: todo!("capability arg"),
            fn_name: StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            payload: HostInputFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            provenance: AgentPubKeyFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        ret
    }
);

/// Fixturator curve for a named zome invocation
/// cell id, test wasm for zome to call, function name, host input payload
pub struct NamedInvocation(pub CellId, pub TestWasm, pub String, pub HostInput);

impl Iterator for ZomeInvocationFixturator<NamedInvocation> {
    type Item = ZomeInvocation;
    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = ZomeInvocationFixturator::new(Unpredictable).next().unwrap();
        ret.cell_id = self.0.curve.0.clone();
        ret.zome_name = self.0.curve.1.clone().into();
        ret.fn_name = self.0.curve.2.clone();
        ret.payload = self.0.curve.3.clone();
        Some(ret)
    }
}

impl Invocation for ZomeInvocation {
    fn allow_side_effects(&self) -> bool {
        true
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome_name.to_owned())
    }
    fn fn_components(&self) -> FnComponents {
        vec![self.fn_name.to_owned()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(self.payload)
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

    fn zomes_to_invoke(&self, zomes_to_invoke: ZomesToInvoke) -> Vec<ZomeName>;

    fn maybe_call<I: Invocation + 'static>(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: &I,
        zome_name: &ZomeName,
        to_call: String,
    ) -> Result<Option<GuestOutput>, RibosomeError>;

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

    fn run_init(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult>;

    fn run_migrate_agent(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult>;

    fn run_validation_package(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackageResult>;

    fn run_post_commit(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<PostCommitResult>;

    /// Helper function for running a validation callback. Just calls
    /// [`run_callback`][] under the hood.
    /// [`run_callback`]: #method.run_callback
    fn run_validate(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult>;

    fn call_iterator<R: 'static + RibosomeT, I: 'static + Invocation>(
        &self,
        workspace: UnsafeInvokeZomeWorkspace,
        ribosome: R,
        invocation: I,
    ) -> CallIterator<R, I>;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        self,
        workspace: UnsafeInvokeZomeWorkspace,
        // TODO: ConductorHandle
        invocation: ZomeInvocation,
    ) -> RibosomeResult<ZomeInvocationResponse>;
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomeInvocationResponse;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use core::time::Duration;
    use holo_hash::holo_hash_core::HeaderHash;
    use holochain_serialized_bytes::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::commit::CommitEntryResult;
    use holochain_zome_types::*;
    use test_wasm_common::TestString;

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

                let workspace = $crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable).next().unwrap();
                let invocation = $crate::core::ribosome::ZomeInvocationFixturator::new(
                    $crate::core::ribosome::NamedInvocation(
                        holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                            .next()
                            .unwrap(),
                        $test_wasm.into(),
                        $fn_name.into(),
                        holochain_zome_types::HostInput::new($input.try_into().unwrap()),
                    ),
                )
                .next()
                .unwrap();
                let zome_invocation_response = ribosome.call_zome_function(workspace, invocation).unwrap();

                // instance building off a warm module should be the slowest part of a wasm test
                // so if each instance (including inner callbacks) takes ~1ms this gives us
                // headroom on 4 call(back)s
                $crate::end_hard_timeout!(timeout, crate::perf::MULTI_WASM_CALL);

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
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "foo".into(),
                HostInput::new(().try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(GuestOutput::new(
                TestString::from(String::from("foo")).try_into().unwrap()
            )),
            ribosome.call_zome_function(workspace, invocation).unwrap()
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
