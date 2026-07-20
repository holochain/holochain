use super::{CallContext, Ribosome};
use holochain_cascade::CascadeImpl;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

/// default size for KeyRefs
const DEF_REF_SIZE: usize = 32;

pub(crate) trait KeyRefExt: Sized {
    fn to_tag(&self) -> Arc<str>;
}

impl KeyRefExt for XSalsa20Poly1305KeyRef {
    fn to_tag(&self) -> Arc<str> {
        let tag = subtle_encoding::base64::encode(self);
        let tag = unsafe { String::from_utf8_unchecked(tag) };
        tag.into_boxed_str().into()
    }
}

pub struct HostFnApi {
    ribosome: Arc<Ribosome>,
    call_context: Arc<CallContext>,
}

impl HostFnApi {
    pub fn new(ribosome: Arc<Ribosome>, call_context: Arc<CallContext>) -> Self {
        Self {
            ribosome,
            call_context,
        }
    }
}

macro_rules! host_fn_api_impls {
    ( $( $(#[cfg(feature = $feat:literal)])? fn $f:ident ( $input:ty ) -> $output:ty; )* ) => {
        $(
            $(#[cfg(feature = $feat)])?
            pub(crate) mod $f;
        )*

        impl HostFnApiT for HostFnApi {
            $(
                $(#[cfg(feature = $feat)])?
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

/// Construct a [CascadeImpl] from a zome call context. It has access to the network
/// and contains the zome name and function name as origin.
pub fn cascade_from_call_context(call_context: &CallContext) -> CascadeImpl {
    CascadeImpl::from_workspace_and_network(
        &call_context.host_context.workspace(),
        call_context.host_context.network().clone(),
    )
    .with_zome_call_origin(call_context.zome.zome_name(), call_context.function_name())
}

// All host_fn_api_impls below rely on this import
use holochain_zome_types as zt;

host_fn_api_impls! {

    // ------------------------------------------------------------------
    // These definitions are copy-pasted from
    // holochain_zome_types::zome_io
    // MAYBE: is there a way to unhygienically import this code in both places?

    // Attempt to accept a preflight request.
    #[cfg(feature = "unstable-countersigning")]
    fn accept_countersigning_preflight_request(zt::prelude::PreflightRequest) -> zt::prelude::PreflightRequestAcceptance;

    // Info about the calling agent.
    fn agent_info (()) -> zt::prelude::AgentInfo;

    // Info about the current DNA.
    fn dna_info_1 (()) -> zt::prelude::DnaInfoV1;
    fn dna_info_2 (()) -> zt::prelude::DnaInfoV2;

    // @todo
    fn call_info (()) -> zt::prelude::CallInfo;

    fn call (Vec<zt::prelude::Call>) -> Vec<zt::prelude::ZomeCallResponse>;

    // @todo List all the local capability claims.
    fn capability_claims (()) -> ();

    // @todo List all the local capability grants.
    fn capability_grants (()) -> ();

    // @todo Get the capability for the current zome call.
    fn capability_info (()) -> ();

    // The EntryDefId determines how a create is handled on the host side.
    // CapGrant and CapClaim are handled natively.
    // App entries are referenced by entry defs then SerializedBytes stuffed into an Entry::App.
    // Returns ActionHash of the newly created record.
    fn create (zt::prelude::CreateInput) -> holo_hash::ActionHash;

    fn x_salsa20_poly1305_shared_secret_create_random(
        Option<zt::prelude::XSalsa20Poly1305KeyRef>
    ) -> zt::prelude::XSalsa20Poly1305KeyRef;

    fn x_salsa20_poly1305_shared_secret_export(
        zt::prelude::XSalsa20Poly1305SharedSecretExport
    ) -> zt::prelude::XSalsa20Poly1305EncryptedData;

    fn x_salsa20_poly1305_shared_secret_ingest(
        zt::prelude::XSalsa20Poly1305SharedSecretIngest
    ) -> zt::prelude::XSalsa20Poly1305KeyRef;

    fn x_salsa20_poly1305_encrypt(
        zt::prelude::XSalsa20Poly1305Encrypt
    ) -> zt::prelude::XSalsa20Poly1305EncryptedData;

    fn x_salsa20_poly1305_decrypt(
        zt::prelude::XSalsa20Poly1305Decrypt
    ) -> Option<zt::prelude::XSalsa20Poly1305Data>;

    fn create_x25519_keypair(()) -> zt::prelude::X25519PubKey;

    // Sender, Recipient, Data.
    fn x_25519_x_salsa20_poly1305_encrypt (zt::prelude::X25519XSalsa20Poly1305Encrypt) -> zt::prelude::XSalsa20Poly1305EncryptedData;

    // Recipient, Sender, Encrypted data.
    fn x_25519_x_salsa20_poly1305_decrypt (zt::prelude::X25519XSalsa20Poly1305Decrypt) -> Option<zt::prelude::XSalsa20Poly1305Data>;

    // Sender, Recipient, Data.
    fn ed_25519_x_salsa20_poly1305_encrypt (zt::prelude::Ed25519XSalsa20Poly1305Encrypt) -> zt::prelude::XSalsa20Poly1305EncryptedData;

    // Recipient, Sender, Encrypted data.
    fn ed_25519_x_salsa20_poly1305_decrypt (zt::prelude::Ed25519XSalsa20Poly1305Decrypt) -> zt::prelude::XSalsa20Poly1305Data;

    // Create a link between two entries.
    fn create_link (zt::prelude::CreateLinkInput) -> holo_hash::ActionHash;

    // Delete an entry.
    fn delete (zt::prelude::DeleteInput) -> holo_hash::ActionHash;

    // Delete a CreateLink record.
    fn delete_link (zt::prelude::DeleteLinkInput) -> holo_hash::ActionHash;

    // Action hash of the newly committed record.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal (zt::prelude::AppSignal) -> ();

    // The trace host import takes a TraceMsg to output wherever the host wants to display it.
    // TraceMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn trace (zt::prelude::TraceMsg) -> ();

    // Attempt to get a live entry from the cascade.
    fn get (Vec<zt::prelude::GetInput>) -> Vec<Option<zt::prelude::Record>>;

    fn get_agent_activity (zt::prelude::GetAgentActivityInput) -> zt::prelude::AgentActivityStatus;

    fn get_details (Vec<zt::prelude::GetInput>) -> Vec<Option<zt::prelude::Details>>;

    // Get links by entry hash from the cascade.
    fn get_links (Vec<zt::prelude::GetLinksInput>) -> Vec<Vec<zt::prelude::Link>>;

    fn get_links_details (Vec<zt::prelude::GetLinksInput>) -> Vec<zt::prelude::LinkDetails>;

    fn count_links(zt::prelude::LinkQuery) -> usize;

    // Retreive a record from the DHT or short circuit.
    fn must_get_valid_record (zt::prelude::MustGetValidRecordInput) -> zt::prelude::Record;

    // Retreive a entry from the DHT or short circuit.
    fn must_get_entry (zt::prelude::MustGetEntryInput) -> zt::prelude::EntryHashed;

    // Retrieve an action from the DHT or short circuit.
    fn must_get_action (zt::prelude::MustGetActionInput) -> zt::prelude::SignedActionHashed;

    // Retrieve an agent activity chain segment from the DHT or short circuit.
    fn must_get_agent_activity (zt::prelude::MustGetAgentActivityInput) -> Vec<zt::prelude::AgentActivity>;

    // Query the source chain for data.
    fn query (zt::prelude::ChainQueryFilter) -> Vec<zt::prelude::Record>;

    // the length of random bytes to create
    fn random_bytes (u32) -> zt::prelude::Bytes;

    // Remotely signal many agents without waiting for responses
    fn send_remote_signal (zt::prelude::RemoteSignal) -> ();

    // Schedule a schedulable function if it is not already scheduled.
    fn schedule (String) -> ();

    // TODO deprecated, remove me
    #[cfg(feature = "unstable-functions")]
    fn sleep (core::time::Duration) -> ();

    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    fn sign (zt::prelude::Sign) -> zt::prelude::Signature;

    // Sign a list of datas with an ephemeral, randomly generated keypair.
    fn sign_ephemeral (zt::prelude::SignEphemeral) -> zt::prelude::EphemeralSignatures;

    // Current system time, in the opinion of the host, as a `Duration`.
    fn sys_time (()) -> zt::prelude::Timestamp;

    // Same as  but also takes the ActionHash of the updated record.
    fn update (zt::prelude::UpdateInput) -> holo_hash::ActionHash;

    fn verify_signature (zt::prelude::VerifySignature) -> bool;

    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info (()) -> zt::prelude::ZomeInfo;

    // Create a clone of an existing cell.
    fn create_clone_cell(zt::prelude::CreateCloneCellInput) -> zt::prelude::ClonedCell;

    // Disable a clone cell.
    fn disable_clone_cell(zt::prelude::DisableCloneCellInput) -> ();

    // Enable a clone cell.
    fn enable_clone_cell(zt::prelude::EnableCloneCellInput) -> zt::prelude::ClonedCell;

    // Delete a clone cell.
    fn delete_clone_cell(zt::clone::DeleteCloneCellInput) -> ();

    // Close your source chain, indicating that you are migrating to a new DNA
    fn close_chain(zt::prelude::CloseChainInput) -> holo_hash::ActionHash;

    // Open your chain, pointing to the previous DNA
    fn open_chain(zt::prelude::OpenChainInput) -> holo_hash::ActionHash;

    // Read the init properties supplied for this cell's role at install time.
    fn get_init_properties(()) -> Option<zt::prelude::InitProperties>;

    // Get validation receipts for an action
    fn get_validation_receipts(zt::prelude::GetValidationReceiptsInput) -> Vec<zt::prelude::ValidationReceiptSet>;
}
