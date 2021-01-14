use crate::cell::CellId;
use crate::prelude as zt;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

macro_rules! wasm_io_type {
    ( $struct:ident($arg:ty) ) => {
        #[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
        pub struct $struct($arg);

        impl $struct {
            pub fn new(i: $arg) -> Self {
                Self(i)
            }

            pub fn into_inner(self) -> $arg {
                self.0
            }

            pub fn inner_ref(&self) -> &$arg {
                &self.0
            }
        }
    };
}

/// All wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way.
macro_rules! wasm_io_types {
    ( $( fn $f:ident ( $in_arg:ty ) -> $out_arg:ty; )* ) => {
        $(
            paste::paste! {
                wasm_io_type!([<$f:camel Input>]($in_arg));
                wasm_io_type!([<$f:camel Output>]($out_arg));

                // Typically we only need this for input types
                impl From<$in_arg> for [<$f:camel Input>] {
                    fn from(arg: $in_arg) -> Self {
                        Self::new(arg)
                    }
                }
            }
        )*

        pub trait HostFnApiT {
            $(
                fn $f(&self, _: $in_arg) -> Result<$out_arg, HostFnApiError>;
            )*
        }
    }
}

// Every externed function that the zome developer exposes to holochain returns `ExternOutput`.
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
wasm_io_type!(ExternInput(SerializedBytes));
wasm_io_type!(ExternOutput(SerializedBytes));

wasm_io_types! {

    // ------------------------------------------------------------------
    // These definitions can be copy-pasted into the ribosome's HostFnApi
    // when updated

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

    fn create_x25519_keypair(()) -> zt::x_salsa20_poly1305::x25519::X25519PubKey;

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

    // Returns HeaderHash of the newly created element.
    fn create (zt::entry::EntryWithDefId) -> holo_hash::HeaderHash;

    // Create a link between two entries.
    fn create_link (zt::link::CreateLinkInputInner) -> holo_hash::HeaderHash;

    // Delete an element.
    fn delete (holo_hash::HeaderHash) -> holo_hash::HeaderHash;

    // Header hash of the CreateLink element.
    fn delete_link (holo_hash::HeaderHash) -> holo_hash::HeaderHash;

    // @todo
    fn entry_type_properties (()) -> ();

    // Header hash of the newly committed element.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal (zt::signal::AppSignal) -> ();

    // The debug host import takes a DebugMsg to output wherever the host wants to display it.
    // DebugMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn debug (zt::debug::DebugMsg) -> ();

    // Attempt to get a live entry from the cascade.
    fn get (zt::entry::GetInputInner) -> Option<zt::element::Element>;

    fn get_agent_activity (zt::agent_info::GetAgentActivityInputInner) -> zt::query::AgentActivity;

    fn get_details (zt::entry::GetInputInner) -> Option<zt::metadata::Details>;

    // Get links by entry hash from the cascade.
    fn get_links (zt::link::GetLinksInputInner) -> zt::link::Links;

    fn get_link_details (zt::link::GetLinksInputInner) -> zt::link::LinkDetails;

    // Hash an entry on the host.
    fn hash_entry (zt::entry::Entry) -> holo_hash::EntryHash;

    // @todo
    fn property (()) -> ();

    // Query the source chain for data.
    fn query (zt::query::ChainQueryFilter) -> zt::element::ElementVec;

    // the length of random bytes to create
    fn random_bytes (u32) -> zt::bytes::Bytes;

    // Remotely signal many agents without waiting for responses
    fn remote_signal (zt::signal::RemoteSignal) -> ();

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
    fn update (zt::entry::UpdateInputInner) -> holo_hash::HeaderHash;

    fn verify_signature (zt::signature::VerifySignature) -> bool;

    // There's nothing to go in or out of a noop.
    // Used to "defuse" host functions when side effects are not allowed.
    fn unreachable (()) -> ();

    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info (()) -> zt::zome_info::ZomeInfo;
}

/// Anything that can go wrong while calling a HostFnApi method
#[derive(thiserror::Error, Debug)]
pub enum HostFnApiError {
    #[error("Error from within host function implementation: {0}")]
    RibosomeError(Box<dyn std::error::Error + Send + Sync>),
}

/// Response to a zome call.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
pub enum ZomeCallResponse {
    /// Arbitrary response from zome fns to the outside world.
    /// Something like a 200 http response.
    Ok(ExternOutput),
    /// Cap grant failure.
    /// Something like a 401 http response.
    Unauthorized(CellId, ZomeName, FunctionName, AgentPubKey),
    /// This was a zome call made remotely but
    /// something has failed on the network
    NetworkError(String),
}
