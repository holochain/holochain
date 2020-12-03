use super::{error::RibosomeError, CallContext, RibosomeT};
use holochain_zome_types::zome_io::HostFnApiT;
use std::sync::Arc;

pub struct HostFnApi<Ribosome: RibosomeT> {
    ribosome: Arc<Ribosome>,
    call_context: Arc<CallContext>,
}

impl<Ribosome: RibosomeT> HostFnApi<Ribosome> {
    pub fn new(ribosome: Arc<Ribosome>, call_context: Arc<CallContext>) -> Self {
        Self {
            ribosome,
            call_context,
        }
    }
}

macro_rules! host_fn_api_impls {
    ( $( fn $f:ident ( $input:ty ) -> $output:ty; )* ) => {
        $(
            pub(crate) mod $f;
        )*

        impl<Ribosome: RibosomeT> HostFnApiT for HostFnApi<Ribosome> {
            type Error = RibosomeError;
            $(
                fn $f(&self, input: $input) -> Result<$output, Self::Error> {
                    $f::$f(
                        self.ribosome.clone(),
                        self.call_context.clone(),
                        input.into()
                    ).map(|r| r.into_inner())
                }
            )*
        }
    };
}

// All host_fn_api_impls below rely on this import
use holochain_zome_types as zt;

host_fn_api_impls! {

    // ------------------------------------------------------------------
    // These definitions are copy-pasted from
    // holochain_zome_types::zome_io
    // TODO: is there a way to unhygienically import this code in both places?

    fn agent_info (()) -> zt::agent_info::AgentInfo;

    fn call (zt::call::Call) -> zt::ZomeCallResponse;

    // Header hash of the DeleteLink element.
    fn call_remote (zt::call_remote::CallRemote) -> zt::ZomeCallResponse;

    // @todo List all the local capability claims.
    fn capability_claims (()) -> ();

    // @todo List all the local capability grants.
    fn capability_grants (()) -> ();

    // @todo Get the capability for the current zome call.
    fn capability_info (()) -> ();

    // The EntryDefId determines how a create is handled on the host side.
    // CapGrant and CapClaim are handled natively.
    // App entries are referenced by entry defs then SerializedBytes stuffed into an Entry::App.
    // Returns HeaderHash of the newly created element.
    fn create ((zt::entry_def::EntryDefId, zt::entry::Entry)) -> holo_hash::HeaderHash;

    // Create a link between two entries.
    fn create_link ((holo_hash::EntryHash, holo_hash::EntryHash, zt::link::LinkTag)) -> holo_hash::HeaderHash;

    // @todo
    fn decrypt (()) -> ();

    // @todo
    fn delete (holo_hash::HeaderHash) -> holo_hash::HeaderHash;

    // Header hash of the CreateLink element.
    fn delete_link (holo_hash::HeaderHash) -> holo_hash::HeaderHash;

    // @todo
    fn encrypt (()) -> ();

    // @todo
    fn entry_type_properties (()) -> ();

    // Header hash of the newly committed element.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal (zt::signal::AppSignal) -> ();

    // The debug host import takes a DebugMsg to output wherever the host wants to display it.
    // DebugMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn debug (zt::debug::DebugMsg) -> ();

    // Attempt to get a live entry from the cascade.
    fn get ((holo_hash::AnyDhtHash, zt::entry::GetOptions)) -> Option<zt::element::Element>;

    fn get_agent_activity (
        (
            holo_hash::AgentPubKey,
            zt::query::ChainQueryFilter,
            zt::query::ActivityRequest,
        )
    ) -> zt::query::AgentActivity;

    fn get_details ((holo_hash::AnyDhtHash, zt::entry::GetOptions)) -> Option<zt::metadata::Details>;

    // Get links by entry hash from the cascade.
    fn get_links ((holo_hash::EntryHash, Option<zt::link::LinkTag>)) -> zt::link::Links;

    fn get_link_details ((holo_hash::EntryHash, Option<zt::link::LinkTag>)) -> zt::link::LinkDetails;

    // Hash an entry on the host.
    fn hash_entry (zt::entry::Entry) -> holo_hash::EntryHash;

    // @todo
    fn property (()) -> ();

    // Query the source chain for data.
    fn query (zt::query::ChainQueryFilter) -> zt::element::ElementVec;

    // the length of random bytes to create
    fn random_bytes (u32) -> zt::bytes::Bytes;

    // // @todo
    // fn send (()) -> ();

    // @todo
    fn schedule (core::time::Duration) -> ();

    // @todo
    fn show_env (()) -> ();

    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    fn sign (zt::signature::Sign) -> zt::signature::Signature;

    // Current system time, in the opinion of the host, as a `Duration`.
    fn sys_time (()) -> core::time::Duration;

    // Same as  but also takes the HeaderHash of the updated element.
    fn update ((zt::entry_def::EntryDefId, zt::entry::Entry, holo_hash::HeaderHash)) -> holo_hash::HeaderHash;

    fn verify_signature (zt::signature::VerifySignature) -> bool;

    // There's nothing to go in or out of a noop.
    // Used to "defuse" host functions when side effects are not allowed.
    fn unreachable (()) -> ();

    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info (()) -> zt::zome_info::ZomeInfo;

}
