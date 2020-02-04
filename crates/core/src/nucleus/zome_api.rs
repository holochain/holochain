use crate::net::NetRequester;
use sx_types::prelude::*;

pub trait ZomeApi {
    fn call();
    fn commit_capability_claim();
    fn commit_capability_grant();
    fn commit_entry();
    fn commit_entry_result();
    fn debug();
    fn decrypt();
    fn emit_signal();
    fn encrypt();
    fn entry_address();
    // fn entry_type_properties();
    fn get_entry();
    // fn get_entry_history();
    // fn get_entry_initial();
    fn get_entry_results();

    fn get_links();
    // et al...

    fn link_entries();
    fn property(); // --> get_property ?
    fn query();
    fn query_result();
    fn remove_link();
    fn send();
    fn sign();
    fn sign_one_time();
    fn sleep();
    fn verify_signature();
    fn remove_entry();
    // fn update_agent();
    fn update_entry();
    fn version();
    fn version_hash();
}

pub struct ZomeEnvironment<N: NetRequester> {
    chain_cursor: Todo,
    net_requester: N,
}

impl<N: NetRequester> ZomeEnvironment<N> {
    pub fn new(chain_cursor: Todo, net_requester: N) -> Self {
        Self {
            chain_cursor,
            net_requester,
        }
    }
}
