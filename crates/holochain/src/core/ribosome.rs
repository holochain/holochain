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

use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::CallbackIterator;
use error::RibosomeResult;
use holochain_types::{
    dna::Dna,
    shims::*,
};
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use holochain_zome_types::validate::ValidationPackage;
use mockall::automock;
use std::iter::Iterator;
use holochain_types::cell::CellId;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::ZomeExternHostInput;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use crate::core::ribosome::host_fn::AllowSideEffects;
use holochain_zome_types::ZomeExternGuestOutput;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
// DO NOT CLONE THIS because payload can be huge
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapToken,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: ZomeExternHostInput,
    /// the provenance of the call
    pub provenance: AgentPubKey,
    /// the hash of the top header at the time of call
    pub as_at: HeaderHash,
}

impl From<&ZomeInvocation> for AllowSideEffects {
    fn from(zome_invocation: &ZomeInvocation) -> Self {
        Self::Yes
    }
}

/// Response to a zome invocation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(ZomeExternGuestOutput),
}

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [WasmRibosome]
#[automock]
pub trait RibosomeT: Sized {
    // ribosomes need a dna
    fn dna(&self) -> &Dna;

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

    fn run_custom_validation_package(
        &self,
        invocation: ValidationPackageInvocation,
    ) -> RibosomeResult<ValidationPackage>;

    fn run_post_commit(
        &self,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<Vec<Option<PostCommitResult>>>;

    /// Helper function for running a validation callback. Just calls
    /// [`run_callback`][] under the hood.
    /// [`run_callback`]: #method.run_callback
    fn run_validate(
        &self,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult>;

    // fn callback_iterator(&self, callback: CallbackInvocation, allow_side_effects: bool) -> (dyn Iterator<Item = RibosomeResult<Option<CallbackGuestOutput>>> + Send + Sized);
    fn callback_iterator<R: 'static + RibosomeT, I: 'static + crate::core::ribosome::guest_callback::Invocation>(&self, ribosome: R, invocation: I) -> CallbackIterator<R, I>;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        // FIXME: Use [SourceChain] instead
        _bundle: &mut SourceChainCommitBundle<'env>,
        invocation: &ZomeInvocation,
    ) -> RibosomeResult<ZomeInvocationResponse>;
}

#[cfg(test)]
pub mod wasm_test {
    use super::WasmRibosome;
    use crate::core::ribosome::RibosomeT;
    use core::time::Duration;
    use holo_hash::holo_hash_core::HeaderHash;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::{
        nucleus::{ZomeInvocation, ZomeInvocationResponse},
        shims::SourceChainCommitBundle,
        test_utils::{fake_agent_pubkey_1, fake_cap_token, fake_cell_id},
    };
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::commit::CommitEntryResult;
    use holochain_zome_types::validate::ValidateEntryResult;
    use holochain_zome_types::*;
    use test_wasm_common::TestString;

    use crate::core::ribosome::HostContext;
    use holochain_types::{
        dna::{wasm::DnaWasm, zome::Zome, Dna},
        test_utils::{fake_dna, fake_header_hash, fake_zome},
    };
    use std::collections::BTreeMap;

    fn zome_from_code(code: DnaWasm) -> Zome {
        let mut zome = fake_zome();
        zome.code = code;
        zome
    }

    fn dna_from_zomes(zomes: BTreeMap<String, Zome>) -> Dna {
        let mut dna = fake_dna("uuid");
        dna.zomes = zomes;
        dna
    }

    pub fn zome_invocation_from_names(
        zome_name: &str,
        fn_name: &str,
        payload: SerializedBytes,
    ) -> ZomeInvocation {
        ZomeInvocation {
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cell_id: fake_cell_id("bob"),
            cap: fake_cap_token(),
            payload: ZomeExternHostInput::new(payload),
            provenance: fake_agent_pubkey_1(),
            as_at: fake_header_hash("fake"),
        }
    }

    pub fn test_ribosome(warm: Option<&str>) -> WasmRibosome {
        // warm the zome module in the module cache
        if let Some(zome_name) = warm {
            let ribosome = test_ribosome(None);
            let _ = ribosome
                .instance(
                    HostContext {
                        zome_name: zome_name.to_string(),
                    },
                    true,
                )
                .unwrap();
        }
        WasmRibosome::new(dna_from_zomes({
            let mut v = std::collections::BTreeMap::new();
            v.insert(String::from("foo"), zome_from_code(TestWasm::Foo.into()));
            v.insert(
                String::from("imports"),
                zome_from_code(TestWasm::Imports.into()),
            );
            v.insert(
                String::from("debug"),
                zome_from_code(TestWasm::Debug.into()),
            );
            v.insert(
                String::from("validate"),
                zome_from_code(TestWasm::Validate.into()),
            );
            v
        }))
    }

    pub fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[macro_export]
    macro_rules! call_test_ribosome {
        ( $zome:literal, $fn_name:literal, $input:expr ) => {
            tokio::task::spawn(async move {
                use crate::core::ribosome::RibosomeT;
                use std::convert::TryFrom;
                use std::convert::TryInto;
                let ribosome = $crate::core::ribosome::wasm_test::test_ribosome(Some($zome));

                let timeout = $crate::start_hard_timeout!();

                let invocation = $crate::core::ribosome::wasm_test::zome_invocation_from_names(
                    $zome,
                    $fn_name,
                    $input.try_into().unwrap(),
                );
                let zome_invocation_response = ribosome
                    .call_zome_function(
                        &mut holochain_types::shims::SourceChainCommitBundle::default(),
                        invocation,
                    )
                    .unwrap();

                // instance building off a warm module should be the slowest part of a wasm test
                // so if each instance (including inner callbacks) takes ~1ms this gives us
                // headroom on 4 call(back)s
                $crate::end_hard_timeout!(timeout, 5_000_000);

                let output = match zome_invocation_response {
                    holochain_types::nucleus::ZomeInvocationResponse::ZomeApiFn(guest_output) => {
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

    #[test]
    fn invoke_foo_test() {
        let ribosome = test_ribosome(Some("foo"));

        let invocation =
            zome_invocation_from_names("foo", "foo", SerializedBytes::try_from(()).unwrap());

        assert_eq!(
            ZomeInvocationResponse::ZomeApiFn(ZomeExternGuestOutput::new(
                TestString::from(String::from("foo")).try_into().unwrap()
            )),
            ribosome
                .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
                .unwrap()
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_test() {
        assert_eq!(
            CommitEntryResult::Success(HeaderHash::new(vec![0xdb; 36])),
            call_test_ribosome!("validate", "always_validates", ()),
        );

        assert_eq!(
            CommitEntryResult::ValidateFailed(ValidateEntryResult::Invalid(
                "NeverValidates never validates".to_string()
            )),
            call_test_ribosome!("validate", "never_validates", ()),
        );
    }
}
