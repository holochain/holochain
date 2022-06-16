use super::CallContext;
use super::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

/// default size for KeyRefs
const DEF_REF_SIZE: usize = 32;

pub(crate) trait KeyRefExt: Sized {
    fn to_tag(&self) -> Arc<str>;
    //fn from_tag<R: AsRef<str>>(tag: R) -> Result<Self, RuntimeError>;
}

impl KeyRefExt for XSalsa20Poly1305KeyRef {
    fn to_tag(&self) -> Arc<str> {
        let tag = subtle_encoding::base64::encode(self);
        let tag = unsafe { String::from_utf8_unchecked(tag) };
        tag.into_boxed_str().into()
    }

    /*
    fn from_tag<R: AsRef<str>>(tag: R) -> Result<Self, RuntimeError> {
        subtle_encoding::base64::decode(tag.as_ref())
            .map_err(|subtle_error| {
                wasm_error!(WasmErrorInner::Host(subtle_error.to_string())).into()
            })
            .map(Into::into)
    }
    */
}

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
            $(
                fn $f(&self, input: $input) -> Result<$output, HostFnApiError> {
                    $f::$f(
                        self.ribosome.clone(),
                        self.call_context.clone(),
                        input.into()
                    ).map_err(|e| HostFnApiError::RibosomeError(Box::new(e)))
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
    // MAYBE: is there a way to unhygienically import this code in both places?

    // Info about the calling agent.
    fn agent_info (()) -> zt::info::AgentInfo;

    // @todo
    fn dna_info (()) -> zt::info::DnaInfo;

    // @todo
    fn call_info (()) -> zt::info::CallInfo;

    fn call (Vec<zt::call::Call>) -> Vec<zt::ZomeCallResponse>;

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
    fn create (zt::entry::CreateInput) -> holo_hash::HeaderHash;

    fn x_salsa20_poly1305_shared_secret_create_random(
        Option<zt::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef>
    ) -> zt::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef;

    fn x_salsa20_poly1305_shared_secret_export(
        zt::x_salsa20_poly1305::XSalsa20Poly1305SharedSecretExport
    ) -> zt::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    fn x_salsa20_poly1305_shared_secret_ingest(
        zt::x_salsa20_poly1305::XSalsa20Poly1305SharedSecretIngest
    ) -> zt::x_salsa20_poly1305::key_ref::XSalsa20Poly1305KeyRef;

    fn x_salsa20_poly1305_encrypt(
        holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Encrypt
    ) -> holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    fn x_salsa20_poly1305_decrypt(
        holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt
    ) -> Option<holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data>;

    fn create_x25519_keypair(()) -> holochain_zome_types::x_salsa20_poly1305::x25519::X25519PubKey;

    // Sender, Recipient, Data.
    fn x_25519_x_salsa20_poly1305_encrypt (holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Encrypt) -> holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    // Recipient, Sender, Encrypted data.
    fn x_25519_x_salsa20_poly1305_decrypt (holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt) -> Option<holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data>;

    // Create a link between two entries.
    fn create_link (zt::link::CreateLinkInput) -> holo_hash::HeaderHash;

    // Delete an entry.
    fn delete (zt::entry::DeleteInput) -> holo_hash::HeaderHash;

    // Delete a CreateLink element.
    fn delete_link (zt::link::DeleteLinkInput) -> holo_hash::HeaderHash;

    // Header hash of the newly committed element.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal (zt::signal::AppSignal) -> ();

    // The trace host import takes a TraceMsg to output wherever the host wants to display it.
    // TraceMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn trace (zt::trace::TraceMsg) -> ();

    // Attempt to get a live entry from the cascade.
    fn get (Vec<zt::entry::GetInput>) -> Vec<Option<zt::element::Element>>;

    fn get_agent_activity (zt::agent_activity::GetAgentActivityInput) -> zt::query::AgentActivity;

    fn get_details (Vec<zt::entry::GetInput>) -> Vec<Option<zt::metadata::Details>>;

    // Get links by entry hash from the cascade.
    fn get_links (Vec<zt::link::GetLinksInput>) -> Vec<Vec<zt::link::Link>>;

    fn get_link_details (Vec<zt::link::GetLinksInput>) -> Vec<zt::link::LinkDetails>;

    // Hash data on the host.
    fn hash (zt::hash::HashInput) -> zt::hash::HashOutput;

    // Retreive an element from the DHT or short circuit.
    fn must_get_valid_element (zt::entry::MustGetValidElementInput) -> Element;

    // Retreive a entry from the DHT or short circuit.
    fn must_get_entry (zt::entry::MustGetEntryInput) -> EntryHashed;

    // Retrieve a header from the DHT or short circuit.
    fn must_get_header (zt::entry::MustGetHeaderInput) -> SignedHeaderHashed;

    // Attempt to accept a preflight request.
    fn accept_countersigning_preflight_request(zt::countersigning::PreflightRequest) -> zt::countersigning::PreflightRequestAcceptance;

    // Query the source chain for data.
    fn query (zt::query::ChainQueryFilter) -> Vec<Element>;

    // the length of random bytes to create
    fn random_bytes (u32) -> zt::bytes::Bytes;

    // Remotely signal many agents without waiting for responses
    fn remote_signal (zt::signal::RemoteSignal) -> ();

    // // @todo
    // fn send (()) -> ();

    // @todo
    fn schedule (String) -> ();

    // @todo
    fn sleep (core::time::Duration) -> ();

    // @todo
    fn version (()) -> zt::version::ZomeApiVersion;

    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    fn sign (zt::signature::Sign) -> zt::signature::Signature;

    // Sign a list of datas with an ephemeral, randomly generated keypair.
    fn sign_ephemeral (zt::signature::SignEphemeral) -> zt::signature::EphemeralSignatures;

    // Current system time, in the opinion of the host, as a `Duration`.
    fn sys_time (()) -> zt::timestamp::Timestamp;

    // Same as  but also takes the HeaderHash of the updated element.
    fn update (zt::entry::UpdateInput) -> holo_hash::HeaderHash;

    fn verify_signature (zt::signature::VerifySignature) -> bool;

    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info (()) -> zt::info::ZomeInfo;

}
