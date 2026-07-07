//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [crate::core::ribosome::real_ribosome::RealRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possibility that we might support applications written in other
//! languages and environments.

use self::guest_callback::{
    entry_defs::EntryDefsInvocation, genesis_self_check::GenesisSelfCheckResult,
};
use self::{
    error::RibosomeError,
    guest_callback::genesis_self_check::{GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation},
};
use crate::conductor::api::CellConductorHandle;
use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::api::ZomeCallParamsSigned;
use crate::conductor::error::ConductorResult;
use crate::core::metrics::ribosome_zome_call_duration_metric;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::{
    GenesisSelfCheckHostAccessV1, GenesisSelfCheckInvocationV1, GenesisSelfCheckResultV1,
};
use crate::core::ribosome::guest_callback::genesis_self_check::v2::{
    GenesisSelfCheckHostAccessV2, GenesisSelfCheckInvocationV2,
};
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::{call_stream, CallStream};
use derive_more::Constructor;
use error::RibosomeResult;
use futures::future::BoxFuture;
use guest_callback::entry_defs::EntryDefsHostAccess;
use guest_callback::init::InitHostAccess;
use guest_callback::post_commit::PostCommitHostAccess;
use guest_callback::validate::ValidateHostAccess;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_nonce::*;
use holochain_p2p::DynHolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::conductor::WitnessNonceResult;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_types::prelude::*;
use holochain_types::zome_types::{GlobalZomeTypes, ZomeTypesError};
use holochain_wasmer_host::error::WasmHostError;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner};
use holochain_zome_types::block::BlockTargetId;
use mockall::automock;
use opentelemetry::KeyValue;
use std::collections::HashMap;
use std::iter::Iterator;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use wasmer::RuntimeError;

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact
pub mod error;

/// How to version guest callbacks.
/// See `genesis_self_check` for an example.
///
/// - Create unversioned structs in the root of the callback module
///   - Invocation, result, host access
///   - The unversioned structs should thinly wrap all their versioned structs
/// - Create versioned submodules for the callback
///   - In these, create versioned structs for the unversioned structs
///   - Write/keep all tests for the versioned copies of the callbacks, test
///     wasms can expose externs directly without the macros for explicit
///     legacy identities if needed.
/// - On the ribosome make sure the trait uses the unversioned struct
///   - Inside the callback method loop over the versioned callbacks, and
///     dispatch each that is found in the wasm
///   - Figure out how to merge/handle results if multiple versions of a callback
///     are found in the target wasm
/// - The ribosome method caller will now be forced by types to provide the
///   unversioned struct, which means they cannot forget to provide and dispatch
///   everything required for each version
/// - Update the `map_extern` macro so that the unversioned name of the callback
///   maps to the latest version of the callback, e.g. `genesis_self_check` is
///   rewritten to `genesis_self_check_2` at the time of writing
///   - This has the effect of newly compiled wasms implementing the callback
///     that is newest when they compile, without polluting the unversioned
///     callback, which is effectively legacy/deprecated behaviour to call it
///     directly.
pub mod guest_callback;

/// How to version host_fns.
/// See `dna_info_1` and `dna_info_2` for an example.
///
/// - Create new versions of the host fn and related IO structs
///   - Any change to an IO struct implies/necessitates a new host fn version
///   - Changes to structs MAY also trigger a new callback version if there is
///     a partially shared data structure in their interfaces
///   - Update the IO type aliases to point to the newest version of all structs
/// - Map both the old and new host functions in the ribosome
/// - Define both of the host functions in the wasm externs in HDI/HDK
/// - Test all versions of every host fn
/// - Ensure the convenience wrapper in the HDI/HDK references the latest version
///   of the host_fn
pub mod host_fn;
pub mod real_ribosome;

mod check_clone_access;

#[cfg(feature = "test_utils")]
pub mod inline_ribosome;
#[cfg(feature = "test_utils")]
pub mod mock_ribosome;

#[derive(Clone)]
pub struct CallContext {
    pub(crate) zome: Zome,
    pub(crate) function_name: FunctionName,
    pub(crate) auth: InvocationAuth,
    pub(crate) host_context: HostContext,
}

impl CallContext {
    pub fn new(
        zome: Zome,
        function_name: FunctionName,
        host_context: HostContext,
        auth: InvocationAuth,
    ) -> Self {
        Self {
            zome,
            function_name,
            host_context,
            auth,
        }
    }

    pub fn zome(&self) -> &Zome {
        &self.zome
    }

    pub fn function_name(&self) -> &FunctionName {
        &self.function_name
    }

    pub fn host_context(&self) -> HostContext {
        self.host_context.clone()
    }

    pub fn auth(&self) -> InvocationAuth {
        self.auth.clone()
    }

    pub fn switch_host_context(
        &self,
        transform: impl Fn(&HostContext) -> Result<HostContext, RuntimeError>,
    ) -> Result<CallContext, RuntimeError> {
        Ok(Self {
            zome: self.zome.clone(),
            function_name: self.function_name.clone(),
            host_context: transform(&self.host_context)?,
            auth: self.auth.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub enum HostContext {
    EntryDefs(EntryDefsHostAccess),
    GenesisSelfCheckV1(GenesisSelfCheckHostAccessV1),
    GenesisSelfCheckV2(GenesisSelfCheckHostAccessV2),
    Init(InitHostAccess),
    PostCommit(PostCommitHostAccess),
    Validate(ValidateHostAccess),
    ZomeCall(ZomeCallHostAccess),
}

impl From<&HostContext> for HostFnAccess {
    fn from(host_access: &HostContext) -> Self {
        match host_access {
            HostContext::ZomeCall(access) => access.into(),
            HostContext::GenesisSelfCheckV1(access) => access.into(),
            HostContext::GenesisSelfCheckV2(access) => access.into(),
            HostContext::Validate(access) => access.into(),
            HostContext::Init(access) => access.into(),
            HostContext::EntryDefs(access) => access.into(),
            HostContext::PostCommit(access) => access.into(),
        }
    }
}

impl HostContext {
    /// Get the workspace, panics if none was provided
    pub fn workspace(&self) -> HostFnWorkspaceRead {
        self.maybe_workspace().expect(
            "Gave access to a host function that uses the workspace without providing a workspace",
        )
    }

    /// Get the workspace if it was provided.
    pub fn maybe_workspace(&self) -> Option<HostFnWorkspaceRead> {
        match self.clone() {
            Self::ZomeCall(ZomeCallHostAccess { workspace, .. })
            | Self::Init(InitHostAccess { workspace, .. })
            | Self::PostCommit(PostCommitHostAccess { workspace, .. }) => Some(workspace.as_read()),
            Self::Validate(ValidateHostAccess { workspace, .. }) => Some(workspace),
            _ => None,
        }
    }

    /// Get the workspace, panics if none was provided
    pub fn workspace_write(&self) -> &HostFnWorkspace {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { workspace, .. })
            | Self::Init(InitHostAccess { workspace, .. })
            | Self::PostCommit(PostCommitHostAccess { workspace, .. }) => workspace,
            _ => panic!(
                "Gave access to a host function that writes to the workspace without providing a workspace"
            ),
        }
    }

    /// Get the keystore, panics if none was provided
    pub fn keystore(&self) -> &MetaLairClient {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { keystore, .. })
            | Self::Init(InitHostAccess { keystore, .. })
            | Self::PostCommit(PostCommitHostAccess { keystore, .. }) => keystore,
            _ => panic!(
                "Gave access to a host function that uses the keystore without providing a keystore"
            ),
        }
    }

    /// Get the network, panics if none was provided
    pub fn network(&self) -> DynHolochainP2pDna {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { network, .. })
            | Self::Init(InitHostAccess { network, .. })
            | Self::PostCommit(PostCommitHostAccess { network, .. }) => network.clone(),
            Self::Validate(ValidateHostAccess { network, .. }) => network.clone(),
            _ => panic!(
                "Gave access to a host function that uses the network without providing a network"
            ),
        }
    }

    /// Get the signal sender, panics if none was provided
    pub fn signal_tx(&mut self) -> &mut broadcast::Sender<Signal> {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { signal_tx, .. })
            | Self::Init(InitHostAccess { signal_tx, .. })
            | Self::PostCommit(PostCommitHostAccess { signal_tx, .. })
            => signal_tx,
            _ => panic!(
                "Gave access to a host function that uses the signal broadcaster without providing one"
            ),
        }
    }

    /// Get the call zome handle, panics if none was provided
    pub fn call_zome_handle(&self) -> &CellConductorReadHandle {
        match self {
            Self::ZomeCall(ZomeCallHostAccess {
                               call_zome_handle, ..
                           })
            | Self::Init(InitHostAccess { call_zome_handle, .. })
            | Self::PostCommit(PostCommitHostAccess { call_zome_handle: Some(call_zome_handle), .. })
            => call_zome_handle,
            _ => panic!(
                "Gave access to a host function that uses the call zome handle without providing a call zome handle"
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FnComponents(pub Vec<String>);

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

impl FnComponents {
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ZomesToInvoke {
    /// All the integrity zomes.
    AllIntegrity,
    /// All integrity and coordinator zomes.
    All,
    /// A single zome of unknown type.
    One(Zome),
    /// A single integrity zome.
    OneIntegrity(IntegrityZome),
    /// A single coordinator zome.
    OneCoordinator(CoordinatorZome),
}

impl ZomesToInvoke {
    pub fn one(zome: Zome) -> Self {
        Self::One(zome)
    }
    pub fn one_integrity(zome: IntegrityZome) -> Self {
        Self::OneIntegrity(zome)
    }
    pub fn one_coordinator(zome: CoordinatorZome) -> Self {
        Self::OneCoordinator(zome)
    }
}

#[derive(Clone, Debug)]
pub enum InvocationAuth {
    LocalCallback,
    Cap(AgentPubKey, Option<CapSecret>),
}

impl InvocationAuth {
    pub fn new(agent_pubkey: AgentPubKey, cap_secret: Option<CapSecret>) -> Self {
        Self::Cap(agent_pubkey, cap_secret)
    }
}

pub trait Invocation: Send + Sync {
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
    /// See [`CallbackResult::is_definitive`] in zome_types.
    /// All of the individual callback results are then folded into a single overall result value
    /// as a From implementation on the invocation results structs (e.g. zome results vs. ribosome
    /// results).
    fn fn_components(&self) -> FnComponents;
    /// The serialized input from the host for the wasm call.
    ///
    /// This is intentionally callable only once because ExternIO may be huge so copies of it
    /// shouldn't be retrieved multiple times.
    fn take_host_input(&self) -> Result<Option<ExternIO>, SerializedBytesError>;
    fn auth(&self) -> InvocationAuth;
}

impl ZomeCallInvocation {
    /// to decide if a zome call grant is authorized:
    /// - we need to find a live (committed and not deleted) cap grant that matches the secret
    /// - if the live cap grant is for the current author the call is ALWAYS authorized ELSE
    /// - the live cap grant needs to include the invocation's provenance AND zome/function name
    pub async fn verify_grant(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> RibosomeResult<ZomeCallAuthorization> {
        let check_function = (self.zome.zome_name().clone(), self.fn_name.clone());
        let check_agent = self.provenance.clone();
        let check_secret = self.cap_secret;
        let maybe_grant: Option<CapGrant> = host_access
            .workspace
            .source_chain()
            .as_ref()
            .expect("Must have source chain to make zome calls")
            .valid_cap_grant(check_function, check_agent, check_secret)
            .await?;
        Ok(if maybe_grant.is_some() {
            ZomeCallAuthorization::Authorized
        } else {
            ZomeCallAuthorization::BadCapGrant
        })
    }

    pub async fn verify_nonce(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> RibosomeResult<ZomeCallAuthorization> {
        Ok(
            match host_access
                .call_zome_handle
                .witness_nonce_from_calling_agent(
                    self.provenance.clone(),
                    self.nonce,
                    self.expires_at,
                )
                .await
                .map_err(Box::new)?
            {
                WitnessNonceResult::Fresh => ZomeCallAuthorization::Authorized,
                nonce_result => ZomeCallAuthorization::BadNonce(format!("{nonce_result:?}")),
            },
        )
    }

    pub async fn verify_blocked_provenance(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> ConductorResult<ZomeCallAuthorization> {
        if host_access
            .call_zome_handle
            .is_blocked(
                BlockTargetId::Cell(CellId::new(
                    (*self.cell_id.dna_hash()).clone(),
                    self.provenance.clone(),
                )),
                Timestamp::now(),
            )
            .await?
        {
            Ok(ZomeCallAuthorization::BlockedProvenance)
        } else {
            Ok(ZomeCallAuthorization::Authorized)
        }
    }

    /// to verify if the zome call is authorized:
    /// - the nonce must not have already been seen
    /// - the grant must be valid
    /// - the provenance must not have any active blocks against them right now
    ///
    /// The checks MUST be done in this order as witnessing the nonce is a write operation,
    /// and so we MUST NOT write nonces until after we verify the signature.
    pub async fn is_authorized(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> ConductorResult<ZomeCallAuthorization> {
        Ok(match self.verify_nonce(host_access).await? {
            ZomeCallAuthorization::Authorized => match self.verify_grant(host_access).await? {
                ZomeCallAuthorization::Authorized => {
                    self.verify_blocked_provenance(host_access).await?
                }
                unauthorized => unauthorized,
            },
            unauthorized => unauthorized,
        })
    }
}

mockall::mock! {
    Invocation {}
    impl Invocation for Invocation {
        fn zomes(&self) -> ZomesToInvoke;
        fn fn_components(&self) -> FnComponents;
        fn take_host_input(&self) -> Result<Option<ExternIO>, SerializedBytesError>;
        fn auth(&self) -> InvocationAuth;
    }
    impl Clone for Invocation {
        fn clone(&self) -> Self;
    }
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCallInvocation {
    /// The Id of the `Cell` in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The Zome containing the function that would be invoked
    pub zome: Zome,
    /// The capability request authorization.
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an Unrestricted status
    /// via a `CapGrant`. Otherwise, it will be necessary to provide a `CapSecret` for every call.
    pub cap_secret: Option<CapSecret>,
    /// The name of the Zome function to call
    pub fn_name: FunctionName,
    /// The serialized data to pass as an argument to the Zome call
    pub payload: Arc<std::sync::Mutex<Option<ExternIO>>>,
    /// The provenance of the call. Provenance means the 'source'
    /// so this expects the `AgentPubKey` of the agent calling the Zome function
    pub provenance: AgentPubKey,
    /// The nonce of the call. Must be unique and monotonic.
    /// If a higher nonce has been seen then older zome calls will be discarded.
    pub nonce: Nonce256Bits,
    /// This call MUST NOT be respected after this time, in the opinion of the callee.
    pub expires_at: Timestamp,
}

impl Invocation for ZomeCallInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome.to_owned())
    }

    fn fn_components(&self) -> FnComponents {
        vec![self.fn_name.to_owned().into()].into()
    }

    fn take_host_input(&self) -> Result<Option<ExternIO>, SerializedBytesError> {
        Ok(self
            .payload
            .lock()
            .unwrap_or_else(|i| i.into_inner())
            .take())
    }

    fn auth(&self) -> InvocationAuth {
        InvocationAuth::Cap(self.provenance.clone(), self.cap_secret)
    }
}

impl ZomeCallInvocation {
    pub async fn try_from_params(
        conductor_api: CellConductorHandle,
        params: ZomeCallParams,
    ) -> RibosomeResult<Self> {
        let ZomeCallParams {
            cap_secret,
            cell_id,
            expires_at,
            fn_name,
            nonce,
            payload,
            provenance,
            zome_name,
        } = params;
        let zome = conductor_api
            .get_zome(&cell_id, &zome_name)
            .map_err(|conductor_api_error| RibosomeError::from(Box::new(conductor_api_error)))?;
        Ok(Self {
            cell_id,
            zome,
            cap_secret,
            fn_name,
            payload: Arc::new(std::sync::Mutex::new(Some(payload))),
            provenance,
            nonce,
            expires_at,
        })
    }
}

#[derive(Clone, Constructor)]
pub struct ZomeCallHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: DynHolochainP2pDna,
    pub signal_tx: broadcast::Sender<Signal>,
    pub call_zome_handle: CellConductorReadHandle,
}

impl std::fmt::Debug for ZomeCallHostAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZomeCallHostAccess").finish()
    }
}

impl From<ZomeCallHostAccess> for HostContext {
    fn from(zome_call_host_access: ZomeCallHostAccess) -> Self {
        Self::ZomeCall(zome_call_host_access)
    }
}

impl From<&ZomeCallHostAccess> for HostFnAccess {
    fn from(_: &ZomeCallHostAccess) -> Self {
        Self::all()
    }
}

pub type DynRibosomeT = Arc<dyn RibosomeImplT>;

/// A ribosome for running hApp code.
///
/// This structure provides the common logic for execution logic and delegates to a [RibosomeImplT]
/// implementation for executing calls.
#[derive(Clone, Debug)]
pub struct Ribosome {
    inner: DynRibosomeT,

    /// The DNA definition, allowing lookups of `ZomeDef`s.
    dna_def: DnaDefHashed,

    /// Entry and link types for each integrity zome.
    ///
    /// Derived from the `dna_def`.
    zome_types: Arc<GlobalZomeTypes>,

    /// Dependencies for every zome
    ///
    /// Derived from the `dna_def`..
    zome_dependencies: Arc<HashMap<ZomeName, Vec<ZomeIndex>>>,
}

impl Ribosome {
    pub async fn new<R: RibosomeImplT + 'static>(
        dna_def: DnaDefHashed,
        ribosome_impl: R,
    ) -> RibosomeResult<Self> {
        let inner = Arc::new(ribosome_impl);
        let mut instance = Self {
            dna_def: dna_def.clone(),
            inner: inner.clone(),
            zome_types: Default::default(),
            zome_dependencies: Default::default(),
        };

        // Collect the number of entry and link types for each integrity zome.
        // TODO: should this be in parallel? Are they all beholden to the same lock?
        let items =
            futures::future::join_all(dna_def.integrity_zomes.iter().map(|(name, zome)| async {
                let zome = Zome::new(name.clone(), zome.clone().erase_type());

                // Call the const functions that return the number of types.
                let num_entry_types = match inner
                    .call_const_fn(
                        Arc::new(instance.clone()),
                        zome.clone(),
                        "__num_entry_types".to_string(),
                    )
                    .await?
                {
                    Some(i) => {
                        let i: u8 = i
                            .try_into()
                            .map_err(|_| ZomeTypesError::EntryTypeIndexOverflow)?;
                        i
                    }
                    None => 0,
                };
                let num_link_types = match inner
                    .call_const_fn(
                        Arc::new(instance.clone()),
                        zome,
                        "__num_link_types".to_string(),
                    )
                    .await?
                {
                    Some(i) => {
                        let i: u8 = i
                            .try_into()
                            .map_err(|_| ZomeTypesError::LinkTypeIndexOverflow)?;
                        i
                    }
                    None => 0,
                };
                RibosomeResult::Ok((num_entry_types, num_link_types))
            }))
            .await
            .into_iter()
            .collect::<RibosomeResult<Vec<_>>>()?;

        // Create the global zome types from the totals.
        let zome_types = GlobalZomeTypes::from_ordered_iterator(items)?;

        // Create a map of integrity zome names to ZomeIndexes.
        let integrity_zomes: HashMap<_, _> = dna_def
            .integrity_zomes
            .iter()
            .enumerate()
            .map(|(i, (n, _))| Some((n.clone(), ZomeIndex(i.try_into().ok()?))))
            .collect::<Option<_>>()
            .ok_or(ZomeTypesError::ZomeIndexOverflow)?;

        // Collect the dependencies for each zome.
        let zome_dependencies = dna_def
            .all_zomes()
            .map(|(zome_name, def)| {
                let mut dependencies = Vec::new();

                // Integrity zomes need to have themselves as a dependency.
                if dna_def.is_integrity_zome(zome_name) {
                    // Get the ZomeIndex for this zome.
                    let id = integrity_zomes.get(zome_name).copied().ok_or_else(|| {
                        ZomeTypesError::MissingDependenciesForZome(zome_name.clone())
                    })?;
                    dependencies.push(id);
                }
                for name in def.dependencies() {
                    // Get the ZomeIndex for this dependency.
                    let id = integrity_zomes.get(name).copied().ok_or_else(|| {
                        ZomeTypesError::MissingDependenciesForZome(zome_name.clone())
                    })?;
                    dependencies.push(id);
                }

                Ok((zome_name.clone(), dependencies))
            })
            .collect::<RibosomeResult<HashMap<_, _>>>()?;

        instance.zome_types = Arc::new(zome_types);
        instance.zome_dependencies = Arc::new(zome_dependencies);

        Ok(instance)
    }

    #[cfg(feature = "test_utils")]
    pub async fn new_with_test_wasms(
        test_wasms: Vec<holochain_wasm_test_utils::TestWasm>,
    ) -> RibosomeResult<Self> {
        let mut dna_def_builder = DnaDefBuilder::default();

        let mut integrity_zomes: IntegrityZomes = vec![];
        let mut coordinator_zomes: CoordinatorZomes = vec![];

        let store = holochain_state::wasm::WasmStore::new(
            holochain_data::test_open_db(holochain_data::kind::Wasm)
                .await
                .unwrap(),
        );
        for test_wasm in test_wasms {
            integrity_zomes.push((
                test_wasm.integrity_zome_name(),
                test_wasm.integrity_zome().def.into(),
            ));
            coordinator_zomes.push((
                test_wasm.coordinator_zome_name(),
                test_wasm.coordinator_zome().def.into(),
            ));

            let dnas: Vec<DnaWasm> = test_wasm.into();
            for dna in dnas {
                store
                    .put(DnaWasmHashed::from_content(dna).await)
                    .await
                    .unwrap();
            }
        }

        let dna_def = dna_def_builder
            .integrity_zomes(integrity_zomes)
            .coordinator_zomes(coordinator_zomes)
            .modifiers(DnaModifiers {
                network_seed: uuid::Uuid::new_v4().to_string(),
                properties: SerializedBytes::default(),
            })
            .build()
            .unwrap();
        let dna_def_hashed = DnaDefHashed::from_content_sync(dna_def);

        let real_ribosome = real_ribosome::RealRibosome::new(
            real_ribosome::WasmBackend::new(),
            dna_def_hashed.clone(),
            Arc::new(real_ribosome::module_cache::make_module_cache(
                real_ribosome::WasmBackend::new(),
                store,
            )),
        )
        .await?;
        Ribosome::new(dna_def_hashed, real_ribosome).await
    }

    pub fn dna_def(&self) -> &DnaDefHashed {
        &self.dna_def
    }

    pub fn zome_types(&self) -> Arc<GlobalZomeTypes> {
        self.zome_types.clone()
    }

    pub fn update_dna_def(
        &mut self,
        mutate: impl FnOnce(DnaDefHashed) -> ZomeResult<DnaDefHashed>,
    ) -> RibosomeResult<()> {
        self.dna_def = mutate(self.dna_def.clone())?;
        self.inner.replace_cached_dna_def(self.dna_def.clone())?;
        Ok(())
    }

    /// Inform this ribosome that genesis is complete.
    ///
    /// This signals that all the zome function calls required to set up this ribosome on first
    /// use have completed and cached data can be released until the ribosome is actually used.
    pub(crate) async fn genesis_complete(&self) {
        self.inner.genesis_complete().await;
    }

    fn zomes_to_invoke(&self, zomes_to_invoke: ZomesToInvoke) -> Vec<Zome> {
        match zomes_to_invoke {
            ZomesToInvoke::AllIntegrity => self
                .dna_def
                .integrity_zomes
                .iter()
                .map(|(n, d)| (n.clone(), d.clone().erase_type()).into())
                .collect(),
            ZomesToInvoke::All => self
                .dna_def
                .all_zomes()
                .map(|(n, d)| (n.clone(), d.clone()).into())
                .collect(),
            ZomesToInvoke::One(zome) => vec![zome],
            ZomesToInvoke::OneIntegrity(zome) => vec![zome.erase_type()],
            ZomesToInvoke::OneCoordinator(zome) => vec![zome.erase_type()],
        }
    }

    pub fn get_integrity_zome(&self, zome_index: &ZomeIndex) -> Option<IntegrityZome> {
        self.dna_def
            .integrity_zomes
            .get(zome_index.0 as usize)
            .cloned()
            .map(|(name, def)| IntegrityZome::new(name, def))
    }

    async fn zome_info(&self, zome: Zome) -> RibosomeResult<ZomeInfo> {
        // Get the dependencies for this zome.
        let zome_dependencies = self
            .zome_dependencies
            .get(zome.zome_name())
            .ok_or_else(|| ZomeTypesError::MissingDependenciesForZome(zome.zome_name().clone()))?;

        // Scope the zome types to these dependencies.
        let zome_types = self.zome_types.in_scope_subset(zome_dependencies);

        Ok(ZomeInfo {
            name: zome.zome_name().clone(),
            id: zome_name_to_id(&self.dna_def, zome.zome_name())
                .expect("Failed to get ID for current zome"),
            properties: SerializedBytes::default(),
            entry_defs: {
                match self
                    .run_entry_defs(EntryDefsHostAccess, EntryDefsInvocation)
                    .await
                    .map_err(|e| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(e.to_string())).into()
                    })? {
                    EntryDefsResult::Err(zome, error_string) => {
                        return Err(RibosomeError::WasmRuntimeError(
                            wasm_error!(WasmErrorInner::Host(format!("{zome}: {error_string}")))
                                .into(),
                        ))
                    }
                    EntryDefsResult::Defs(defs) => {
                        let vec = zome_dependencies
                            .iter()
                            .filter_map(|zome_index| {
                                self.dna_def.integrity_zomes.get(zome_index.0 as usize)
                            })
                            .flat_map(|(zome_name, _)| {
                                defs.get(zome_name).map(|e| e.0.clone()).unwrap_or_default()
                            })
                            .collect::<Vec<_>>();
                        vec.into()
                    }
                }
            },
            extern_fns: self.inner.list_zome_fns(zome.zome_name())?,
            zome_types,
        })
    }

    pub async fn maybe_call(
        &self,
        host_context: HostContext,
        invocation: Arc<dyn Invocation + 'static>,
        zome: &Zome,
        fn_name: &FunctionName,
    ) -> Result<Option<ExternIO>, RibosomeError> {
        let this = Arc::new(self.clone());
        let inner = self.inner.clone();
        let dna_hash = self.dna_def.hash.as_hash().to_string();

        let zome = zome.clone();
        let fn_name = fn_name.clone();
        let f = tokio::spawn(async move {
            let mut attributes = vec![
                KeyValue::new("dna_hash", dna_hash),
                KeyValue::new("zome", zome.zome_name().to_string()),
                KeyValue::new("fn", fn_name.to_string()),
            ];

            if let Some(agent_pubkey) = host_context.maybe_workspace().and_then(|workspace| {
                workspace
                    .source_chain()
                    .as_ref()
                    .map(|source_chain| source_chain.agent_pubkey().to_string())
            }) {
                attributes.push(KeyValue::new("agent", agent_pubkey));
            }

            let call_context = CallContext {
                zome: zome.clone(),
                function_name: fn_name.clone(),
                host_context,
                auth: invocation.auth(),
            };

            inner
                .maybe_call(this, call_context, invocation, zome, fn_name, attributes)
                .await
        });

        f.await?
    }

    fn call_stream(
        &self,
        host_context: HostContext,
        invocation: Arc<dyn Invocation + 'static>,
    ) -> CallStream {
        let (s, _h) = call_stream(host_context, self.clone(), invocation);
        s
    }

    async fn do_callback<A, CR, R>(
        &self,
        access: A,
        invocation: Arc<dyn Invocation + 'static>,
    ) -> RibosomeResult<R>
    where
        A: Into<HostContext>,
        CR: CallbackResult + std::fmt::Debug + serde::de::DeserializeOwned,
        R: From<Vec<(ZomeName, CR)>>,
    {
        use tokio_stream::StreamExt;
        let mut results: Vec<(ZomeName, CR)> = Vec::new();
        // fallible iterator syntax instead of for loop
        let mut call_stream = self.call_stream(access.into(), invocation);
        loop {
            let (zome_name, callback_result): (ZomeName, CR) = match call_stream.next().await {
                Some(Ok((zome, extern_io))) => match extern_io.decode() {
                    Ok(callback_result) => (zome.into(), callback_result),
                    Err(SerializedBytesError::Deserialize(err_msg)) => {
                        // Error returned when deserialization fails due to an invalid return type
                        return Err(RibosomeError::CallbackInvalidReturnType(err_msg));
                    }
                    Err(e) => return Err(RibosomeError::WasmRuntimeError(wasm_error!(e).into())),
                },
                Some(Err((zome, RibosomeError::WasmRuntimeError(runtime_error)))) => {
                    let wasm_error: WasmError = runtime_error.downcast()?;
                    if let WasmErrorInner::Deserialize(_) = wasm_error.error {
                        // Error returned when callback called via ribosome with invalid parameters
                        return Err(RibosomeError::CallbackInvalidParameters(String::default()));
                    }

                    (
                        zome.into(),
                        <CR>::try_from_wasm_error(wasm_error)
                            .map_err(|e| -> RuntimeError { WasmHostError(e).into() })?,
                    )
                }
                Some(Err((
                    _zome,
                    RibosomeError::InlineZomeError(InlineZomeError::SerializationError(
                        SerializedBytesError::Deserialize(err_msg),
                    )),
                ))) => {
                    // Error returned when callback called via zome call with invalid parameters
                    return Err(RibosomeError::CallbackInvalidParameters(err_msg));
                }
                Some(Err((_zome, other_error))) => return Err(other_error),
                None => {
                    break;
                }
            };
            // return early if we have a definitive answer, no need to keep invoking callbacks
            // if we know we are done
            if callback_result.is_definitive() {
                return Ok(vec![(zome_name, callback_result)].into());
            }
            results.push((zome_name, callback_result));
        }
        // fold all the non-definitive callbacks down into a single overall result
        Ok(results.into())
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub async fn call_zome_function(
        &self,
        host_access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse> {
        // make a copy of these for the error handling below
        let zome_name = invocation.zome.zome_name().clone();
        let fn_name = invocation.fn_name.clone();

        let start = std::time::Instant::now();
        let attributes = vec![
            opentelemetry::KeyValue::new("dna_hash", self.dna_def.hash.to_string()),
            opentelemetry::KeyValue::new("zome", zome_name.to_string()),
            opentelemetry::KeyValue::new("fn", fn_name.to_string()),
        ];

        let guest_output: ExternIO = match self
            .call_stream(host_access.into(), Arc::new(invocation))
            .next()
            .await
        {
            None => return Err(RibosomeError::ZomeFnNotExists(zome_name, fn_name)),
            Some(Ok((_zome, extern_io))) => extern_io,
            Some(Err((_zome, ribosome_error))) => return Err(ribosome_error),
        };

        // Record call zome duration.
        let elapsed = start.elapsed().as_secs_f64();
        ribosome_zome_call_duration_metric().record(elapsed, &attributes);

        Ok(ZomeCallResponse::Ok(guest_output))
    }

    /// Post commit works a bit different to the other callbacks.
    /// As it is dispatched from a spawned task there is nothing to handle any
    /// result, good or bad, other than to maybe log some error.
    pub async fn run_post_commit(
        &self,
        host_access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<()> {
        match self
            .call_stream(host_access.into(), Arc::new(invocation))
            .next()
            .await
        {
            Some(Ok(_)) | None => Ok(()),
            Some(Err((_zome, ribosome_error))) => Err(ribosome_error),
        }
    }

    async fn run_genesis_self_check_v1(
        &self,
        host_access: GenesisSelfCheckHostAccessV1,
        invocation: GenesisSelfCheckInvocationV1,
    ) -> RibosomeResult<GenesisSelfCheckResultV1> {
        self.do_callback(host_access, Arc::new(invocation)).await
    }

    async fn run_genesis_self_check_v2(
        &self,
        host_access: GenesisSelfCheckHostAccessV2,
        invocation: GenesisSelfCheckInvocationV2,
    ) -> RibosomeResult<GenesisSelfCheckResultV1> {
        self.do_callback(host_access, Arc::new(invocation)).await
    }

    pub async fn run_genesis_self_check(
        &self,
        host_access: GenesisSelfCheckHostAccess,
        invocation: GenesisSelfCheckInvocation,
    ) -> RibosomeResult<GenesisSelfCheckResult> {
        let (invocation_v1, invocation_v2): (
            GenesisSelfCheckInvocationV1,
            GenesisSelfCheckInvocationV2,
        ) = invocation.into();
        let (host_access_v1, host_access_v2): (
            GenesisSelfCheckHostAccessV1,
            GenesisSelfCheckHostAccessV2,
        ) = host_access.into();
        match self
            .run_genesis_self_check_v1(host_access_v1, invocation_v1)
            .await
        {
            Ok(GenesisSelfCheckResultV1::Valid) => Ok(self
                .run_genesis_self_check_v2(host_access_v2, invocation_v2)
                .await?
                .into()),
            result => Ok(result?.into()),
        }
    }

    pub async fn run_validate(
        &self,
        host_access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult> {
        self.do_callback(host_access, Arc::new(invocation)).await
    }

    pub async fn run_init(
        &self,
        host_access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult> {
        self.do_callback(host_access, Arc::new(invocation)).await
    }

    pub async fn run_entry_defs(
        &self,
        host_access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult> {
        self.do_callback(host_access, Arc::new(invocation)).await
    }

    #[cfg(feature = "test_utils")]
    pub fn is_memory_cached(&self, zome_name: &ZomeName) -> RibosomeResult<bool> {
        self.inner.is_memory_cached(zome_name)
    }

    #[cfg(feature = "test_utils")]
    pub fn is_compiled_wasm_stored(
        &self,
        zome_name: ZomeName,
    ) -> BoxFuture<'static, RibosomeResult<bool>> {
        self.inner.is_compiled_wasm_stored(zome_name)
    }
}

fn zome_name_to_id(dna_def: &DnaDefHashed, zome_name: &ZomeName) -> RibosomeResult<ZomeIndex> {
    match dna_def.all_zomes().position(|(name, _)| name == zome_name) {
        Some(index) => Ok(ZomeIndex::from(index as u8)),
        None => Err(RibosomeError::ZomeNotExists(zome_name.to_owned())),
    }
}

/// Interface for a Ribosome implementation.
///
/// Allows either WASM or inline ribosomes to be used with the [`Ribosome`] type.
#[automock]
pub trait RibosomeImplT: std::fmt::Debug + Send + Sync {
    fn maybe_call(
        &self,
        ribosome: Arc<Ribosome>,
        call_context: CallContext,
        invocation: Arc<dyn Invocation + 'static>,
        zome: Zome,
        to_call: FunctionName,
        attributes: Vec<KeyValue>,
    ) -> BoxFuture<'static, Result<Option<ExternIO>, RibosomeError>>;

    /// Get a value from a const wasm function.
    ///
    /// This is really a stand in until Rust can properly support
    /// const wasm values.
    ///
    /// This allows getting values from wasm without the need for any translation.
    /// The same technique can be used with the wasmer cli to validate these
    /// values without needing to make holochain a dependency.
    fn call_const_fn(
        &self,
        ribosome: Arc<Ribosome>,
        zome: Zome,
        name: String,
    ) -> BoxFuture<'_, Result<Option<i32>, RibosomeError>>;

    /// List the exported functions for the named zome
    fn list_zome_fns(&self, zome_name: &ZomeName) -> RibosomeResult<Vec<FunctionName>>;

    /// Replace any cached [`DnaDef`] in the ribosome implementation.
    ///
    /// The [`Ribosome`] holds the authoritative copy of the [`DnaDef`] and permits mutation.
    /// Notify the implementation that the value has changed and it should replace any copy it
    /// holds.
    fn replace_cached_dna_def(&self, dna_def: DnaDefHashed) -> RibosomeResult<()>;

    /// Inform this ribosome that genesis is complete.
    fn genesis_complete(&self) -> BoxFuture<'static, ()> {
        Box::pin(async move {})
    }

    #[cfg(feature = "test_utils")]
    fn is_memory_cached(&self, zome_name: &ZomeName) -> RibosomeResult<bool> {
        let _zome_name = zome_name;
        Ok(false)
    }

    #[cfg(feature = "test_utils")]
    fn is_compiled_wasm_stored(
        &self,
        zome_name: ZomeName,
    ) -> BoxFuture<'static, RibosomeResult<bool>> {
        let _zome_name = zome_name;

        Box::pin(async move { Ok(false) })
    }
}

/// Placeholder for weighing. Currently produces zero weight.
pub fn weigh_placeholder() -> EntryRateWeight {
    EntryRateWeight::default()
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::ribosome::FnComponents;
    use crate::test_utils;
    use core::time::Duration;
    use hdk::prelude::*;
    use holochain_nonce::fresh_nonce;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::zome_io::ZomeCallParams;

    pub fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn verify_zome_call_test() {
        holochain_trace::test_run();
        let test_utils::RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob_pubkey,
            ..
        } = test_utils::RibosomeTestFixture::new(TestWasm::Capability).await;

        let now = Timestamp::now();
        let (nonce, expires_at) = fresh_nonce(now).unwrap();
        let alice_zome_call_params = ZomeCallParams {
            provenance: alice_pubkey.clone(),
            cell_id: alice.cell_id().clone(),
            zome_name: TestWasm::Capability.coordinator_zome_name(),
            fn_name: "needs_cap_claim".into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
            nonce,
            expires_at,
        };

        // Bob observes or forges a valid zome call from alice.
        // He removes Alice's signature but leaves her provenance and adds his own signature.
        let mut bob_zome_call_params = alice_zome_call_params.clone();
        bob_zome_call_params.provenance = bob_pubkey.clone();

        // The call should fail for bob.
        let bob_call_result = conductor.raw_handle().call_zome(bob_zome_call_params).await;

        match bob_call_result {
            Ok(Ok(ZomeCallResponse::Unauthorized(..))) => { /* (☞ ͡° ͜ʖ ͡°)☞ */ }
            _ => panic!("{bob_call_result:?}"),
        }

        // The call should NOT fail for alice (e.g. bob's forgery should not consume alice's nonce).
        let alice_call_result_0 = conductor
            .raw_handle()
            .call_zome(alice_zome_call_params.clone())
            .await;

        match alice_call_result_0 {
            Ok(Ok(ZomeCallResponse::Ok(_))) => { /* ಥ‿ಥ */ }
            _ => panic!("{alice_call_result_0:?}"),
        }

        // The same call cannot be used a second time.
        let alice_call_result_1 = conductor
            .raw_handle()
            .call_zome(alice_zome_call_params)
            .await;

        match alice_call_result_1 {
            Ok(Ok(ZomeCallResponse::Unauthorized(..))) => { /* ☜(ﾟヮﾟ☜) */ }
            _ => panic!("{bob_call_result:?}"),
        }
    }

    #[test]
    fn fn_components_iterate() {
        let fn_components = FnComponents::from(vec!["foo".into(), "bar".into(), "baz".into()]);
        let expected = vec!["foo_bar_baz", "foo_bar", "foo"];

        assert_eq!(fn_components.into_iter().collect::<Vec<String>>(), expected,);
    }
}
