use crate as zt;
use crate::prelude::*;
use holo_hash::AgentPubKey;
pub use holochain_integrity_types::zome_io::*;
use holochain_nonce::Nonce256Bits;

/// All wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way.
macro_rules! wasm_io_types {
    ( $( $(#[cfg(feature = $feat:literal)])? fn $f:ident ( $in_arg:ty ) -> $out_arg:ty; )* ) => {
        pub trait HostFnApiT {
            $(
                $(#[cfg(feature = $feat)])?
                fn $f(&self, _: $in_arg) -> Result<$out_arg, HostFnApiError>;
            )*
        }
    }
}

// Every externed function that the zome developer exposes to holochain returns `ExternIO`.
// The zome developer can expose callbacks in a "sparse" way based on names and the functions
// can take different input (e.g. validation vs. hooks like init, etc.).
// All we can say is that some SerializedBytes are being received and returned.
// In the case of ZomeExtern functions exposed to a client, the data input/output is entirely
// arbitrary so we can't say anything at all. In this case the happ developer must BYO
// deserialization context to match the client, either directly or via. the HDK.
// Note though, that _unlike_ zome externs, the host _does_ know exactly the guest should be
// returning for callbacks, it's just that the unpacking of the return happens in two steps:
// - first the sparse callback is triggered with SB input/output
// - then the guest inflates the expected input or the host the expected output based on the
//   callback flavour

wasm_io_types! {

    // ------------------------------------------------------------------
    // These definitions can be copy-pasted into the ribosome's HostFnApi
    // when updated

    // Attempt to accept a preflight request.
    #[cfg(feature = "unstable-countersigning")]
    fn accept_countersigning_preflight_request(zt::countersigning::PreflightRequest) -> zt::countersigning::PreflightRequestAcceptance;

    // Info about the calling agent.
    fn agent_info (()) -> zt::info::AgentInfo;

    // Block some agent on the same DNA.
    #[cfg(feature = "unstable-functions")]
    fn block_agent (zt::block::BlockAgentInput) -> ();

    // Info about the current DNA.
    fn dna_info_1 (()) -> zt::info::DnaInfoV1;
    fn dna_info_2 (()) -> zt::info::DnaInfoV2;

    // @todo
    fn call_info (()) -> zt::info::CallInfo;

    fn call (Vec<zt::call::Call>) -> Vec<zt::prelude::ZomeCallResponse>;

    // @todo List all the local capability claims.
    fn capability_claims (()) -> ();

    // @todo List all the local capability grants.
    fn capability_grants (()) -> ();

    // @todo Get the capability for the current zome call.
    fn capability_info (()) -> ();

    // Returns ActionHash of the newly created record.
    fn create (zt::entry::CreateInput) -> holo_hash::ActionHash;

    // Create a link between two entries.
    fn create_link (zt::link::CreateLinkInput) -> holo_hash::ActionHash;

    fn create_x25519_keypair(()) -> zt::x_salsa20_poly1305::x25519::X25519PubKey;

    // The debug host import takes a TraceMsg to output wherever the host wants to display it.
    // TraceMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn trace (zt::trace::TraceMsg) -> ();

    // Action hash of the CreateLink record.
    fn delete_link (zt::link::DeleteLinkInput) -> holo_hash::ActionHash;

    // Delete a record.
    fn delete (zt::entry::DeleteInput) -> holo_hash::ActionHash;

    // Action hash of the newly committed record.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal (zt::signal::AppSignal) -> ();

    fn get_agent_activity (zt::agent_activity::GetAgentActivityInput) -> zt::query::AgentActivity;

    fn get_details (Vec<zt::entry::GetInput>) -> Vec<Option<zt::metadata::Details>>;

    fn get_links_details (Vec<zt::link::GetLinksInput>) -> Vec<zt::link::LinkDetails>;

    // Get links by entry hash from the cascade.
    fn get_links (Vec<zt::link::GetLinksInput>) -> Vec<Vec<zt::link::Link>>;

    fn count_links(zt::query::LinkQuery) -> usize;

    // Attempt to get a live entry from the cascade.
    fn get (Vec<zt::entry::GetInput>) -> Vec<Option<zt::record::Record>>;

    // Retreive a record from the DHT or short circuit.
    fn must_get_valid_record (zt::entry::MustGetValidRecordInput) -> zt::record::Record;

    // Retreive a entry from the DHT or short circuit.
    fn must_get_entry (zt::entry::MustGetEntryInput) -> zt::entry::EntryHashed;

    // Retrieve an action from the DHT or short circuit.
    fn must_get_action (zt::entry::MustGetActionInput) -> zt::prelude::SignedActionHashed;

    fn must_get_agent_activity (zt::chain::MustGetAgentActivityInput) -> Vec<zt::op::RegisterAgentActivity>;

    // Query the source chain for data.
    fn query (zt::query::ChainQueryFilter) -> Vec<crate::prelude::Record>;

    // the length of random bytes to create
    fn random_bytes (u32) -> zt::bytes::Bytes;

    // Remotely signal many agents without waiting for responses
    fn send_remote_signal (zt::signal::RemoteSignal) -> ();

    // Schedule a schedulable function if it is not already.
    #[cfg(feature = "unstable-functions")]
    fn schedule (String) -> ();

    // TODO deprecated, remove me
    #[cfg(feature = "unstable-functions")]
    fn sleep (core::time::Duration) -> ();

    // @todo
    fn version (()) -> zt::version::ZomeApiVersion;

    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    fn sign (zt::signature::Sign) -> zt::signature::Signature;

    fn sign_ephemeral (zt::signature::SignEphemeral) -> zt::signature::EphemeralSignatures;

    // Current system time, in the opinion of the host, as a `Timestamp`.
    fn sys_time (()) -> zt::timestamp::Timestamp;

    // Same as  but also takes the ActionHash of the updated record.
    fn update (zt::entry::UpdateInput) -> holo_hash::ActionHash;

    // Unblock some previously blocked agent.
    #[cfg(feature = "unstable-functions")]
    fn unblock_agent(zt::block::BlockAgentInput) -> ();

    fn verify_signature (zt::signature::VerifySignature) -> bool;

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
        zt::x_salsa20_poly1305::XSalsa20Poly1305Encrypt
    ) -> zt::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    fn x_salsa20_poly1305_decrypt(
        zt::x_salsa20_poly1305::XSalsa20Poly1305Decrypt
    ) -> Option<zt::x_salsa20_poly1305::data::XSalsa20Poly1305Data>;

    // Sender, Recipient, Data.
    fn x_25519_x_salsa20_poly1305_encrypt(zt::x_salsa20_poly1305::X25519XSalsa20Poly1305Encrypt) -> zt::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    // Recipient, Sender, Encrypted data.
    fn x_25519_x_salsa20_poly1305_decrypt(zt::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt) -> Option<zt::x_salsa20_poly1305::data::XSalsa20Poly1305Data>;

    // Sender, Recipient, Data.
    fn ed_25519_x_salsa20_poly1305_encrypt(zt::x_salsa20_poly1305::Ed25519XSalsa20Poly1305Encrypt) -> zt::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;

    // Recipient, Sender, Encrypted data.
    fn ed_25519_x_salsa20_poly1305_decrypt(zt::x_salsa20_poly1305::Ed25519XSalsa20Poly1305Decrypt) -> zt::x_salsa20_poly1305::data::XSalsa20Poly1305Data;

    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info (()) -> zt::info::ZomeInfo;

    // Create a clone of an existing cell.
    fn create_clone_cell(zt::clone::CreateCloneCellInput) -> zt::clone::ClonedCell;

    // Disable a clone cell.
    fn disable_clone_cell(zt::clone::DisableCloneCellInput) -> ();

    // Enable a clone cell.
    fn enable_clone_cell(zt::clone::EnableCloneCellInput) -> zt::clone::ClonedCell;

    // Delete a clone cell.
    fn delete_clone_cell(zt::clone::DeleteCloneCellInput) -> ();

    // Close your source chain, indicating that you are migrating to a new DNA
    fn close_chain(zt::chain::CloseChainInput) -> holo_hash::ActionHash;

    // Open your chain, pointing to the previous DNA
    fn open_chain(zt::chain::OpenChainInput) -> holo_hash::ActionHash;

    // Get validation receipts for an action
    fn get_validation_receipts(zt::validate::GetValidationReceiptsInput) -> Vec<zt::validate::ValidationReceiptSet>;
}

/// Anything that can go wrong while calling a HostFnApi method
#[derive(thiserror::Error, Debug)]
pub enum HostFnApiError {
    #[error("Error from within host function implementation: {0}")]
    RibosomeError(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZomeCallAuthorization {
    Authorized,
    BadCapGrant,
    BadNonce(String),
    BlockedProvenance,
}

impl std::fmt::Display for ZomeCallAuthorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ZomeCallAuthorization {
    pub fn is_authorized(&self) -> bool {
        matches!(self, ZomeCallAuthorization::Authorized)
    }
}

/// Response to a zome call.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
pub enum ZomeCallResponse {
    /// Arbitrary response from zome fns to the outside world.
    /// Something like a 200 http response.
    Ok(ExternIO),
    /// Authentication failure - signature could not be verified by the provenance.
    AuthenticationFailed(Signature, AgentPubKey),
    /// Cap grant failure.
    /// Something like a 401 http response.
    Unauthorized(
        ZomeCallAuthorization,
        Option<CapSecret>,
        ZomeName,
        FunctionName,
    ),
    /// This was a zome call made remotely but
    /// something has failed on the network
    NetworkError(String),
    /// A countersigning session has failed to start.
    CountersigningSession(String),
}

impl std::fmt::Display for ZomeCallResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(feature = "test_utils")]
impl ZomeCallResponse {
    pub fn unwrap(self) -> ExternIO {
        match self {
            ZomeCallResponse::Ok(output) => output,
            _ => panic!("Attempted to unwrap a non-Ok ZomeCallResponse"),
        }
    }
}

/// Zome calls need to be signed regardless of how they are called.
/// This defines exactly what needs to be signed.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZomeCallParams {
    /// Provenance to sign.
    pub provenance: AgentPubKey,
    /// Cell ID to sign.
    pub cell_id: CellId,
    /// Zome name to sign.
    pub zome_name: ZomeName,
    /// Function name to sign.
    pub fn_name: FunctionName,
    /// Cap secret to sign.
    pub cap_secret: Option<CapSecret>,
    /// Payload to sign.
    pub payload: ExternIO,
    /// Nonce to sign.
    pub nonce: Nonce256Bits,
    /// Time after which this zome call MUST NOT be accepted.
    pub expires_at: Timestamp,
}

impl ZomeCallParams {
    /// Prepare the canonical bytes for zome call parameters so that they are
    /// always signed and verified in the same way.
    /// Signature is generated for the hash of the bytes.
    pub fn serialize_and_hash(&self) -> Result<(Vec<u8>, Vec<u8>), SerializedBytesError> {
        let bytes = holochain_serialized_bytes::encode(&self)?;
        let bytes_hash = sha2_512(&bytes);
        Ok((bytes, bytes_hash))
    }
}
