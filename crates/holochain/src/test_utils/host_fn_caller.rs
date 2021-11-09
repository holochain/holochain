#![allow(missing_docs)]

use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::api::CellConductorReadHandle;
use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::ribosome::host_fn;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use hdk::prelude::EntryError;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::HolochainP2pRefToCell;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;
use holochain_zome_types::AgentActivity;
use std::sync::Arc;
use tracing::*;
use unwrap_to::unwrap_to;

// Commit entry types //
// Useful for when you want to commit something
// that will match entry defs
pub const POST_ID: &str = "post";
pub const MSG_ID: &str = "msg";
pub const VALID_ID: &str = "always_validates";
pub const INVALID_ID: &str = "never_validates";

#[derive(
    Default, Debug, PartialEq, Clone, SerializedBytes, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Post(pub String);
#[derive(
    Default, Debug, PartialEq, Clone, SerializedBytes, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Msg(pub String);

/// A CellId plus ZomeName: the full "path" to a zome in the conductor
#[derive(Clone, Debug, derive_more::From, derive_more::Into)]
pub struct ZomePath(CellId, ZomeName);

impl ZomePath {
    pub fn cell_id(&self) -> &CellId {
        &self.0
    }

    pub fn zome_name(&self) -> &ZomeName {
        &self.1
    }
}

/// Type from the validate wasm
// TODO: Maybe we can dry this up by putting the wasm types
// somewhere outside the wasm?
#[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
pub enum ThisWasmEntry {
    AlwaysValidates,
    NeverValidates,
}

#[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
pub enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

/// A freely callable version of the host fn api, so that host functions
/// can be called from Rust instead of Wasm
#[derive(Clone)]
pub struct HostFnCaller {
    pub env: EnvWrite,
    pub cache: EnvWrite,
    pub ribosome: RealRibosome,
    pub zome_path: ZomePath,
    pub network: HolochainP2pCell,
    pub keystore: MetaLairClient,
    pub signal_tx: SignalBroadcaster,
    pub call_zome_handle: CellConductorReadHandle,
}

impl HostFnCaller {
    /// Create HostFnCaller for the first zome.
    // #[deprecated = "use create_for_zome"]
    pub async fn create(
        cell_id: &CellId,
        handle: &ConductorHandle,
        dna_file: &DnaFile,
    ) -> HostFnCaller {
        Self::create_for_zome(cell_id, handle, dna_file, 0).await
    }

    /// Create HostFnCaller for a specific zome if there are multiple.
    pub async fn create_for_zome(
        cell_id: &CellId,
        handle: &ConductorHandle,
        dna_file: &DnaFile,
        zome_index: usize,
    ) -> HostFnCaller {
        let env = handle.get_cell_env(cell_id).unwrap();
        let cache = handle.get_cache_env(cell_id).unwrap();
        let keystore = env.keystore().clone();
        let network = handle
            .holochain_p2p()
            .to_cell(cell_id.dna_hash().clone(), cell_id.agent_pubkey().clone());

        let zome_path = (
            cell_id.clone(),
            dna_file.dna().zomes.get(zome_index).unwrap().0.clone(),
        )
            .into();
        let ribosome = RealRibosome::new(dna_file.clone());
        let signal_tx = handle.signal_broadcaster().await;
        let call_zome_handle =
            CellConductorApi::new(handle.clone(), cell_id.clone()).into_call_zome_handle();
        HostFnCaller {
            env,
            cache,
            ribosome,
            zome_path,
            network,
            keystore,
            signal_tx,
            call_zome_handle,
        }
    }

    pub fn env(&self) -> EnvWrite {
        self.env.clone()
    }

    pub async fn unpack(
        &self,
    ) -> (
        EnvWrite,
        Arc<RealRibosome>,
        Arc<CallContext>,
        HostFnWorkspace,
    ) {
        let HostFnCaller {
            env,
            cache,
            network,
            keystore,
            ribosome,
            signal_tx,
            zome_path,
            call_zome_handle,
        } = self.clone();

        let (cell_id, zome_name) = zome_path.into();

        let workspace_lock =
            HostFnWorkspace::new(env.clone(), cache, cell_id.agent_pubkey().clone())
                .await
                .unwrap();
        let host_access = ZomeCallHostAccess::new(
            workspace_lock.clone(),
            keystore,
            network,
            signal_tx,
            call_zome_handle,
        );
        let ribosome = Arc::new(ribosome);
        let zome = ribosome.dna_def().get_zome(&zome_name).unwrap();
        let call_context = Arc::new(CallContext::new(
            zome,
            FunctionName::new("not_sure_what_should_be_here"),
            host_access.into(),
            // Auth as the author.
            InvocationAuth::Cap(cell_id.agent_pubkey().clone(), None),
        ));
        (env, ribosome, call_context, workspace_lock)
    }
}

impl HostFnCaller {
    pub async fn commit_entry<E: Into<entry_def::EntryDefId>>(
        &self,
        entry: Entry,
        entry_def_id: E,
    ) -> HeaderHash {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = CreateInput::new(entry_def_id.into(), entry, ChainTopOrdering::default());
        let output = host_fn::create::create(ribosome, call_context, input).unwrap();

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn delete_entry<'env>(&self, input: DeleteInput) -> HeaderHash {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let output = {
            let r = host_fn::delete::delete(ribosome, call_context, input);
            let r = r.map_err(|e| {
                debug!(%e);
                e
            });
            r.unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn update_entry<'env, E: Into<entry_def::EntryDefId>>(
        &self,
        entry: Entry,
        entry_def_id: E,
        original_header_hash: HeaderHash,
    ) -> HeaderHash {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = UpdateInput::new(
            original_header_hash,
            CreateInput::new(entry_def_id.into(), entry, ChainTopOrdering::default()),
        );
        let output = { host_fn::update::update(ribosome, call_context, input).unwrap() };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get(&self, entry_hash: AnyDhtHash, options: GetOptions) -> Vec<Option<Element>> {
        let (_, ribosome, call_context, _) = self.unpack().await;
        let input = GetInput::new(entry_hash, options);
        host_fn::get::get(ribosome, call_context, vec![input]).unwrap()
    }

    pub async fn get_details<'env>(
        &self,
        entry_hash: AnyDhtHash,
        options: GetOptions,
    ) -> Vec<Option<Details>> {
        let (_, ribosome, call_context, _) = self.unpack().await;
        let input = GetInput::new(entry_hash, options);
        host_fn::get_details::get_details(ribosome, call_context, vec![input]).unwrap()
    }

    pub async fn create_link<'env>(
        &self,
        base: EntryHash,
        target: EntryHash,
        link_tag: LinkTag,
    ) -> HeaderHash {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = CreateLinkInput::new(base, target, link_tag, ChainTopOrdering::default());
        let output = { host_fn::create_link::create_link(ribosome, call_context, input).unwrap() };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn delete_link<'env>(&self, link_add_hash: HeaderHash) -> HeaderHash {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let output = {
            host_fn::delete_link::delete_link(
                ribosome,
                call_context,
                DeleteLinkInput::new(link_add_hash, ChainTopOrdering::default()),
            )
            .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get_links<'env>(
        &self,
        base: EntryHash,
        link_tag: Option<LinkTag>,
        _options: GetLinksOptions,
    ) -> Vec<Link> {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = GetLinksInput::new(base, link_tag);
        let output = {
            host_fn::get_links::get_links(ribosome, call_context, vec![input])
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output
    }

    pub async fn get_link_details<'env>(
        &self,
        base: EntryHash,
        tag: LinkTag,
        _options: GetLinksOptions,
    ) -> Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)> {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;
        let input = GetLinksInput::new(base, Some(tag));
        let output = {
            host_fn::get_link_details::get_link_details(ribosome, call_context, vec![input])
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();

        output.into()
    }

    pub async fn get_agent_activity(
        &self,
        agent: &AgentPubKey,
        query: &ChainQueryFilter,
        request: ActivityRequest,
    ) -> AgentActivity {
        let (_, ribosome, call_context, _) = self.unpack().await;
        let input = GetAgentActivityInput::new(agent.clone(), query.clone(), request);
        host_fn::get_agent_activity::get_agent_activity(ribosome, call_context, input).unwrap()
    }

    pub async fn call_zome_direct(&self, invocation: ZomeCallInvocation) -> ExternIO {
        let (_, ribosome, call_context, workspace_lock) = self.unpack().await;

        let output = {
            let host_access = call_context.host_context();
            let zcha = unwrap_to!(host_access => HostContext::ZomeCall).clone();
            ribosome.call_zome_function(zcha, invocation).unwrap()
        };

        // Write
        workspace_lock.flush(&self.network).await.unwrap();
        unwrap_to!(output => ZomeCallResponse::Ok).to_owned()
    }
}

macro_rules! test_entry_impl {
    ($type:ident) => {
        impl TryFrom<$type> for Entry {
            type Error = EntryError;
            fn try_from(n: $type) -> Result<Self, Self::Error> {
                Ok(Entry::App(SerializedBytes::try_from(n)?.try_into()?))
            }
        }

        impl TryFrom<Entry> for $type {
            type Error = SerializedBytesError;
            fn try_from(entry: Entry) -> Result<Self, Self::Error> {
                let entry = unwrap_to!(entry => Entry::App).clone();
                $type::try_from(entry.into_sb())
            }
        }
    };
}

test_entry_impl!(ThisWasmEntry);
test_entry_impl!(Post);
test_entry_impl!(Msg);
test_entry_impl!(MaybeLinkable);
