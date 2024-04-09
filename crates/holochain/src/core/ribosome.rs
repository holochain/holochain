//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [crate::core::ribosome::real_ribosome::RealRibosome]. The abstract trait exists
//! so that we can write mocks against the `RibosomeT` interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact
#[allow(missing_docs)]
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

use crate::conductor::api::CellConductorHandle;
use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::api::ZomeCall;
use crate::conductor::interface::SignalBroadcaster;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentInvocation;
use crate::core::ribosome::guest_callback::migrate_agent::MigrateAgentResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::CallIterator;
use derive_more::Constructor;
use error::RibosomeResult;
use guest_callback::entry_defs::EntryDefsHostAccess;
use guest_callback::init::InitHostAccess;
use guest_callback::migrate_agent::MigrateAgentHostAccess;
use guest_callback::post_commit::PostCommitHostAccess;
use guest_callback::validate::ValidateHostAccess;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_nonce::*;
use holochain_p2p::HolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::nonce::WitnessNonceResult;
use holochain_types::prelude::*;
use holochain_types::zome_types::GlobalZomeTypes;
use holochain_zome_types::block::BlockTargetId;
use mockall::automock;
use std::iter::Iterator;
use std::sync::Arc;

use self::guest_callback::{
    entry_defs::EntryDefsInvocation, genesis_self_check::GenesisSelfCheckResult,
};
use self::{
    error::RibosomeError,
    guest_callback::genesis_self_check::{GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation},
};

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
}

#[derive(Clone, Debug)]
pub enum HostContext {
    EntryDefs(EntryDefsHostAccess),
    GenesisSelfCheckV1(GenesisSelfCheckHostAccessV1),
    GenesisSelfCheckV2(GenesisSelfCheckHostAccessV2),
    Init(InitHostAccess),
    MigrateAgent(MigrateAgentHostAccess),
    PostCommit(PostCommitHostAccess), // MAYBE: add emit_signal access here?
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
            HostContext::MigrateAgent(access) => access.into(),
            HostContext::PostCommit(access) => access.into(),
        }
    }
}

impl HostContext {
    /// Get the workspace, panics if none was provided
    pub fn workspace(&self) -> HostFnWorkspaceRead {
        match self.clone() {
            Self::ZomeCall(ZomeCallHostAccess { workspace, .. })
            | Self::Init(InitHostAccess { workspace, .. })
            | Self::MigrateAgent(MigrateAgentHostAccess { workspace, .. })
            | Self::PostCommit(PostCommitHostAccess { workspace, .. }) => workspace.into(),
            Self::Validate(ValidateHostAccess { workspace, .. }) => workspace,
            _ => panic!(
                "Gave access to a host function that uses the workspace without providing a workspace"
            ),
        }
    }

    /// Get the workspace, panics if none was provided
    pub fn workspace_write(&self) -> &HostFnWorkspace {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { workspace, .. })
            | Self::Init(InitHostAccess { workspace, .. })
            | Self::MigrateAgent(MigrateAgentHostAccess { workspace, .. })
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
    pub fn network(&self) -> &HolochainP2pDna {
        match self {
            Self::ZomeCall(ZomeCallHostAccess { network, .. })
            | Self::Init(InitHostAccess { network, .. })
            | Self::PostCommit(PostCommitHostAccess { network, .. })
            | Self::Validate(ValidateHostAccess { network, .. }) => network,
            _ => panic!(
                "Gave access to a host function that uses the network without providing a network"
            ),
        }
    }

    /// Get the signal broadcaster, panics if none was provided
    pub fn signal_tx(&mut self) -> &mut SignalBroadcaster {
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
#[cfg_attr(test, derive(arbitrary::Arbitrary))]
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

pub trait Invocation: Clone {
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
    /// See [ `CallbackResult::is_definitive` ] in zome_types.
    /// All of the individual callback results are then folded into a single overall result value
    /// as a From implementation on the invocation results structs (e.g. zome results vs. ribosome
    /// results).
    fn fn_components(&self) -> FnComponents;
    /// the serialized input from the host for the wasm call
    /// this is intentionally NOT a reference to self because ExternIO may be huge we want to be
    /// careful about cloning invocations
    fn host_input(self) -> Result<ExternIO, SerializedBytesError>;
    fn auth(&self) -> InvocationAuth;
}

impl ZomeCallInvocation {
    pub async fn verify_signature(&self) -> RibosomeResult<ZomeCallAuthorization> {
        Ok(
            if self
                .provenance
                .verify_signature_raw(
                    &self.signature,
                    ZomeCallUnsigned::from(ZomeCall::from(self.clone())).data_to_sign()?,
                )
                .await?
            {
                ZomeCallAuthorization::Authorized
            } else {
                ZomeCallAuthorization::BadSignature
            },
        )
    }

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
                nonce_result => ZomeCallAuthorization::BadNonce(format!("{:?}", nonce_result)),
            },
        )
    }

    pub async fn verify_blocked_provenance(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> RibosomeResult<ZomeCallAuthorization> {
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
    /// - the signature must be valid
    /// - the nonce must not have already been seen
    /// - the grant must be valid
    /// - the provenance must not have any active blocks against them right now
    /// the checks MUST be done in this order as witnessing the nonce is a write
    /// and so we MUST NOT write nonces until after we verify the signature.
    #[allow(clippy::extra_unused_lifetimes)]
    pub async fn is_authorized<'a>(
        &self,
        host_access: &ZomeCallHostAccess,
    ) -> RibosomeResult<ZomeCallAuthorization> {
        Ok(match self.verify_signature().await? {
            ZomeCallAuthorization::Authorized => match self.verify_nonce(host_access).await? {
                ZomeCallAuthorization::Authorized => match self.verify_grant(host_access).await? {
                    ZomeCallAuthorization::Authorized => {
                        self.verify_blocked_provenance(host_access).await?
                    }
                    unauthorized => unauthorized,
                },
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
        fn host_input(self) -> Result<ExternIO, SerializedBytesError>;
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
    pub payload: ExternIO,
    /// The provenance of the call. Provenance means the 'source'
    /// so this expects the `AgentPubKey` of the agent calling the Zome function
    pub provenance: AgentPubKey,
    /// The signature of the call from the provenance of the call.
    /// Everything except the signature itself is signed.
    pub signature: Signature,
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
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        Ok(self.payload)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::Cap(self.provenance.clone(), self.cap_secret)
    }
}

impl ZomeCallInvocation {
    pub async fn try_from_interface_call(
        conductor_api: CellConductorHandle,
        call: ZomeCall,
    ) -> RibosomeResult<Self> {
        let ZomeCall {
            cell_id,
            zome_name,
            fn_name,
            cap_secret,
            payload,
            provenance,
            signature,
            nonce,
            expires_at,
        } = call;
        let zome = conductor_api
            .get_zome(cell_id.dna_hash(), &zome_name)
            .map_err(|conductor_api_error| RibosomeError::from(Box::new(conductor_api_error)))?;
        Ok(Self {
            cell_id,
            zome,
            cap_secret,
            fn_name,
            payload,
            provenance,
            signature,
            nonce,
            expires_at,
        })
    }
}

impl From<ZomeCallInvocation> for ZomeCall {
    fn from(inv: ZomeCallInvocation) -> Self {
        let ZomeCallInvocation {
            cell_id,
            zome,
            fn_name,
            cap_secret,
            payload,
            provenance,
            signature,
            nonce,
            expires_at,
        } = inv;
        Self {
            cell_id,
            zome_name: zome.zome_name().clone(),
            fn_name,
            cap_secret,
            payload,
            provenance,
            signature,
            nonce,
            expires_at,
        }
    }
}

#[derive(Clone, Constructor)]
pub struct ZomeCallHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: HolochainP2pDna,
    pub signal_tx: SignalBroadcaster,
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

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [`RealRibosome`](crate::core::ribosome::real_ribosome::RealRibosome)
#[automock]
pub trait RibosomeT: Sized + std::fmt::Debug + Send + Sync {
    fn dna_def(&self) -> &DnaDefHashed;

    fn dna_hash(&self) -> &DnaHash;

    fn dna_file(&self) -> &DnaFile;

    fn zome_info(&self, zome: Zome) -> RibosomeResult<ZomeInfo>;

    fn zomes_to_invoke(&self, zomes_to_invoke: ZomesToInvoke) -> Vec<Zome> {
        match zomes_to_invoke {
            ZomesToInvoke::AllIntegrity => self
                .dna_def()
                .integrity_zomes
                .iter()
                .map(|(n, d)| (n.clone(), d.clone().erase_type()).into())
                .collect(),
            ZomesToInvoke::All => self
                .dna_def()
                .all_zomes()
                .map(|(n, d)| (n.clone(), d.clone()).into())
                .collect(),
            ZomesToInvoke::One(zome) => vec![zome],
            ZomesToInvoke::OneIntegrity(zome) => vec![zome.erase_type()],
            ZomesToInvoke::OneCoordinator(zome) => vec![zome.erase_type()],
        }
    }

    fn zome_name_to_id(&self, zome_name: &ZomeName) -> RibosomeResult<ZomeIndex> {
        match self
            .dna_def()
            .all_zomes()
            .position(|(name, _)| name == zome_name)
        {
            Some(index) => Ok(holochain_zome_types::action::ZomeIndex::from(index as u8)),
            None => Err(RibosomeError::ZomeNotExists(zome_name.to_owned())),
        }
    }

    fn get_integrity_zome(&self, zome_index: &ZomeIndex) -> Option<IntegrityZome>;

    fn call_iterator<I: Invocation + 'static>(
        &self,
        host_context: HostContext,
        invocation: I,
    ) -> CallIterator<Self, I>;

    fn maybe_call<I: Invocation + 'static>(
        &self,
        host_context: HostContext,
        invocation: &I,
        zome: &Zome,
        to_call: &FunctionName,
    ) -> Result<Option<ExternIO>, RibosomeError>;

    /// Get a value from a const wasm function.
    ///
    /// This is really a stand in until Rust can properly support
    /// const wasm values.
    ///
    /// This allows getting values from wasm without the need for any translation.
    /// The same technique can be used with the wasmer cli to validate these
    /// values without needing to make holochain a dependency.
    fn get_const_fn(&self, zome: &Zome, name: &str) -> Result<Option<i32>, RibosomeError>;

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

    fn run_genesis_self_check(
        &self,
        access: GenesisSelfCheckHostAccess,
        invocation: GenesisSelfCheckInvocation,
    ) -> RibosomeResult<GenesisSelfCheckResult>;

    fn run_init(
        &self,
        access: InitHostAccess,
        invocation: InitInvocation,
    ) -> RibosomeResult<InitResult>;

    fn run_migrate_agent(
        &self,
        access: MigrateAgentHostAccess,
        invocation: MigrateAgentInvocation,
    ) -> RibosomeResult<MigrateAgentResult>;

    fn run_entry_defs(
        &self,
        access: EntryDefsHostAccess,
        invocation: EntryDefsInvocation,
    ) -> RibosomeResult<EntryDefsResult>;

    fn run_post_commit(
        &self,
        access: PostCommitHostAccess,
        invocation: PostCommitInvocation,
    ) -> RibosomeResult<()>;

    /// Helper function for running a validation callback. Calls
    /// private fn `do_callback!` under the hood.
    fn run_validate(
        &self,
        access: ValidateHostAccess,
        invocation: ValidateInvocation,
    ) -> RibosomeResult<ValidateResult>;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        &self,
        access: ZomeCallHostAccess,
        invocation: ZomeCallInvocation,
    ) -> RibosomeResult<ZomeCallResponse>;

    fn zome_types(&self) -> &Arc<GlobalZomeTypes>;
}

/// Placeholder for weighing. Currently produces zero weight.
pub fn weigh_placeholder() -> EntryRateWeight {
    EntryRateWeight::default()
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::ribosome::FnComponents;
    use crate::core::ribosome::ZomeCall;
    use crate::sweettest::SweetAgents;
    use crate::sweettest::SweetCell;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::SweetZome;
    use crate::test_utils::host_fn_caller::HostFnCaller;
    use core::time::Duration;
    use hdk::prelude::*;
    use holo_hash::AgentPubKey;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_nonce::fresh_nonce;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::zome_io::ZomeCallUnsigned;

    pub fn now() -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn verify_zome_call_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Foo).await;

        let now = Timestamp::now();
        let (nonce, expires_at) = fresh_nonce(now).unwrap();
        let alice_unsigned_zome_call = ZomeCallUnsigned {
            provenance: alice_pubkey.clone(),
            cell_id: alice.cell_id().clone(),
            zome_name: "foo".into(),
            fn_name: "foo".into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
            nonce,
            expires_at,
        };
        let alice_signed_zome_call = ZomeCall::try_from_unsigned_zome_call(
            &conductor.keystore(),
            alice_unsigned_zome_call.clone(),
        )
        .await
        .unwrap();

        // Bob observes or forges a valid zome call from alice.
        // He removes Alice's signature but leaves her provenance and adds his own signature.
        let mut bob_signed_zome_call = alice_signed_zome_call.clone();
        bob_signed_zome_call.signature = bob_pubkey
            .sign_raw(
                &conductor.keystore(),
                alice_unsigned_zome_call.data_to_sign().unwrap(),
            )
            .await
            .unwrap();

        // The call should fail for bob.
        let bob_call_result = conductor.raw_handle().call_zome(bob_signed_zome_call).await;

        match bob_call_result {
            Ok(Ok(ZomeCallResponse::Unauthorized(_, _, _, _, _))) => { /* (☞ ͡° ͜ʖ ͡°)☞ */
            }
            _ => panic!("{:?}", bob_call_result),
        }

        // The call should NOT fail for alice (e.g. bob's forgery should not consume alice's nonce).
        let alice_call_result_0 = conductor
            .raw_handle()
            .call_zome(alice_signed_zome_call.clone())
            .await;

        match alice_call_result_0 {
            Ok(Ok(ZomeCallResponse::Ok(_))) => { /* ಥ‿ಥ */ }
            _ => panic!("{:?}", alice_call_result_0),
        }

        // The same call cannot be used a second time.
        let alice_call_result_1 = conductor
            .raw_handle()
            .call_zome(alice_signed_zome_call)
            .await;

        match alice_call_result_1 {
            Ok(Ok(ZomeCallResponse::Unauthorized(_, _, _, _, _))) => { /* ☜(ﾟヮﾟ☜) */ }
            _ => panic!("{:?}", bob_call_result),
        }
    }

    #[test]
    fn fn_components_iterate() {
        let fn_components = FnComponents::from(vec!["foo".into(), "bar".into(), "baz".into()]);
        let expected = vec!["foo_bar_baz", "foo_bar", "foo"];

        assert_eq!(fn_components.into_iter().collect::<Vec<String>>(), expected,);
    }

    pub struct RibosomeTestFixture {
        pub conductor: SweetConductor,
        pub alice_pubkey: AgentPubKey,
        pub bob_pubkey: AgentPubKey,
        pub alice: SweetZome,
        pub bob: SweetZome,
        pub alice_cell: SweetCell,
        pub bob_cell: SweetCell,
        pub alice_host_fn_caller: HostFnCaller,
        pub bob_host_fn_caller: HostFnCaller,
    }

    impl RibosomeTestFixture {
        pub async fn new(test_wasm: TestWasm) -> Self {
            let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![test_wasm]).await;

            let mut conductor = SweetConductor::from_standard_config().await;
            let (alice_pubkey, bob_pubkey) = SweetAgents::alice_and_bob();

            let apps = conductor
                .setup_app_for_agents("app-", [&alice_pubkey, &bob_pubkey], [&dna_file])
                .await
                .unwrap();

            let ((alice_cell,), (bob_cell,)) = apps.into_tuples();

            let alice_host_fn_caller = HostFnCaller::create_for_zome(
                alice_cell.cell_id(),
                &conductor.raw_handle(),
                &dna_file,
                0,
            )
            .await;

            let bob_host_fn_caller = HostFnCaller::create_for_zome(
                bob_cell.cell_id(),
                &conductor.raw_handle(),
                &dna_file,
                0,
            )
            .await;

            let alice = alice_cell.zome(test_wasm);
            let bob = bob_cell.zome(test_wasm);

            Self {
                conductor,
                alice_pubkey,
                bob_pubkey,
                alice,
                bob,
                alice_cell,
                bob_cell,
                alice_host_fn_caller,
                bob_host_fn_caller,
            }
        }
    }
}
