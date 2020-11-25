use super::{error::RibosomeResult, CallContext, RibosomeT};
use std::sync::Arc;

pub struct HostFnApi<Ribosome: RibosomeT> {
    ribosome: Arc<Ribosome>,
    call_context: Arc<CallContext>,
}

macro_rules! host_fn_api_impls {
    ( $( fn $f:ident ( $input:ty ) -> $output:ty; )* ) => {
        $(
            pub mod $f;
        )*

        // TODO: impl HostFnApiT for __

        impl<Ribosome: RibosomeT> HostFnApi<Ribosome> {
            $(
                fn $f(&self, input: $input) -> RibosomeResult<$output> {
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

use holochain_zome_types as z;

pub mod get_links;
pub mod hash_entry;
pub mod property;
pub mod query;
pub mod random_bytes;
pub mod schedule;
pub mod show_env;
pub mod sign;
pub mod sys_time;
pub mod unreachable;
pub mod update;
pub mod verify_signature;
pub mod zome_info;

host_fn_api_impls! {
    fn agent_info(()) -> z::agent_info::AgentInfo;
    fn call(z::call::Call) -> z::ZomeCallResponse;
    fn call_remote(z::call_remote::CallRemote) -> z::ZomeCallResponse;
    fn capability_claims(()) -> ();
    fn capability_grants(()) -> ();
    fn capability_info(()) -> ();
    fn create((z::entry_def::EntryDefId, z::entry::Entry)) -> holo_hash::HeaderHash;
    fn create_link((holo_hash::EntryHash, holo_hash::EntryHash, z::link::LinkTag)) -> holo_hash::HeaderHash;
    fn debug(z::debug::DebugMsg) -> ();
    fn decrypt(()) -> ();
    fn delete(holo_hash::HeaderHash) -> holo_hash::HeaderHash;
    fn delete_link(holo_hash::HeaderHash) -> holo_hash::HeaderHash;
    fn emit_signal(z::signal::AppSignal) -> ();
    fn encrypt(()) -> ();
    fn entry_type_properties(()) -> ();
    fn get((holo_hash::AnyDhtHash, z::entry::GetOptions)) -> Option<z::element::Element>;
    fn get_agent_activity((
        holo_hash::AgentPubKey,
        z::query::ChainQueryFilter,
        z::query::ActivityRequest,
    )) -> z::query::AgentActivity;
    fn get_details((holo_hash::AnyDhtHash, z::entry::GetOptions)) -> Option<z::metadata::Details>;
    fn get_link_details((holo_hash::EntryHash, Option<z::link::LinkTag>)) -> z::link::LinkDetails;
    // fn get_links((holo_hash::EntryHash, Option<link::LinkTag>)) -> ;
    // fn hash_entry() -> ;
    // fn property() -> ;
    // fn query() -> ;
    // fn random_bytes() -> ;
    // fn schedule() -> ;
    // fn show_env() -> ;
    // fn sign() -> ;
    // fn sys_time() -> ;
    // fn unreachable() -> ;
    // fn update() -> ;
    // fn verify_signature() -> ;
    // fn zome_info() -> ;
}
