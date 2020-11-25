use crate::*;
use element::ElementVec;
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
    ( $( fn $f:ident $in_struct:ident($in_arg:ty) -> $out_struct:ident($out_arg:ty); )* ) => {
        $(
            wasm_io_type!($in_struct($in_arg));
            wasm_io_type!($out_struct($out_arg));

            // Typically we only need this for input types
            impl From<$in_arg> for $in_struct {
                fn from(arg: $in_arg) -> Self {
                    Self::new(arg)
                }
            }
        )*

        pub trait HostFnApiT {
            $(
                fn $f(_: $in_arg) -> $out_arg;
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

wasm_io_types!(
    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    fn zome_info ZomeInfoInput(()) -> ZomeInfoOutput(zome_info::ZomeInfo);
    fn agent_info AgentInfoInput(()) -> AgentInfoOutput(agent_info::AgentInfo);
    fn call CallInput(call::Call) -> CallOutput(ZomeCallResponse);
    // @todo List all the local capability claims.
    fn capability_claims CapabilityClaimsInput(()) -> CapabilityClaimsOutput(());
    // @todo List all the local capability grants.
    fn capability_grants CapabilityGrantsInput(()) -> CapabilityGrantsOutput(());
    // @todo Get the capability for the current zome call.
    fn capability_info CapabilityInfoInput(()) -> CapabilityInfoOutput(());
    // The EntryDefId determines how a create is handled on the host side.
    // CapGrant and CapClaim are handled natively.
    // App entries are referenced by entry defs then SerializedBytes stuffed into an Entry::App.
    fn create CreateInput((entry_def::EntryDefId, entry::Entry)) -> CreateOutput(holo_hash::HeaderHash);
    // Header hash of the newly created element.
    // @todo
    fn decrypt DecryptInput(()) -> DecryptOutput(());
    // @todo
    fn encrypt EncryptInput(()) -> EncryptOutput(());
    // @todo
    fn showenv ShowEnvInput(()) -> ShowEnvOutput(());
    // @todo
    fn property PropertyInput(()) -> PropertyOutput(());
    // Query the source chain for data.
    fn query QueryInput(query::ChainQueryFilter) -> QueryOutput(ElementVec);
    // the length of random bytes to create
    fn random_bytes RandomBytesInput(u32) -> RandomBytesOutput(bytes::Bytes);
    // Header hash of the CreateLink element.
    fn delete_link DeleteLinkInput(holo_hash::HeaderHash) -> DeleteLinkOutput(holo_hash::HeaderHash);
    // Header hash of the DeleteLink element.
    fn call_remote CallRemoteInput(call_remote::CallRemote) -> CallRemoteOutput(ZomeCallResponse);
    // @todo
    fn send SendInput(()) -> SendOutput(());
    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    fn sign SignInput(crate::signature::Sign) -> SignOutput(crate::signature::Signature);
    fn verify_signature VerifySignatureInput(crate::signature::VerifySignature) -> VerifySignatureOutput(bool);
    // @todo
    fn schedule ScheduleInput(core::time::Duration) -> ScheduleOutput(());
    // Same as CreateInput but also takes the HeaderHash of the updated element.
    fn update UpdateInput((entry_def::EntryDefId, entry::Entry, holo_hash::HeaderHash)) -> UpdateOutput(holo_hash::HeaderHash);
    // Header hash of the newly committed element.
    // Emit a Signal::App to subscribers on the interface
    fn emit_signal EmitSignalInput(signal::AppSignal) -> EmitSignalOutput(());
    // @todo
    fn delete DeleteInput(holo_hash::HeaderHash) -> DeleteOutput(holo_hash::HeaderHash);
    // Create a link between two entries.
    fn create_link CreateLinkInput((holo_hash::EntryHash, holo_hash::EntryHash, link::LinkTag)) -> CreateLinkOutput(holo_hash::HeaderHash);
    // Get links by entry hash from the cascade.
    fn get_links GetLinksInput((holo_hash::EntryHash, Option<link::LinkTag>)) -> GetLinksOutput(link::Links);
    fn get_link_details GetLinkDetailsInput((holo_hash::EntryHash, Option<link::LinkTag>)) -> GetLinkDetailsOutput(link::LinkDetails);
    // Attempt to get a live entry from the cascade.
    fn get GetInput((holo_hash::AnyDhtHash, entry::GetOptions)) -> GetOutput(Option<element::Element>);
    fn get_details GetDetailsInput((holo_hash::AnyDhtHash, entry::GetOptions)) -> GetDetailsOutput(Option<metadata::Details>);
    fn get_agent_activity GetAgentActivityInput(
        (
            holo_hash::AgentPubKey,
            query::ChainQueryFilter,
            query::ActivityRequest,
        )
    ) -> GetAgentActivityOutput(query::AgentActivity);
    // @todo
    fn entry_type_properties EntryTypePropertiesInput(()) -> EntryTypePropertiesOutput(());
    // Hash an entry on the host.
    fn hash_entry HashEntryInput(entry::Entry) -> HashEntryOutput(holo_hash::EntryHash);
    // Current system time, in the opinion of the host, as a `Duration`.
    fn sys_time SysTimeInput(()) -> SysTimeOutput(core::time::Duration);
    // The debug host import takes a DebugMsg to output wherever the host wants to display it.
    // DebugMsg includes line numbers. so the wasm tells the host about it's own code structure.
    fn debug DebugInput(debug::DebugMsg) -> DebugOutput(());
    // There's nothing to go in or out of a noop.
    // Used to "defuse" host functions when side effects are not allowed.
    fn unreachable UnreachableInput(()) -> UnreachableOutput(());
);

/// Response to a zome call.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
pub enum ZomeCallResponse {
    /// Arbitrary response from zome fns to the outside world.
    /// Something like a 200 http response.
    Ok(ExternOutput),
    /// Cap grant failure.
    /// Something like a 401 http response.
    Unauthorized,
    /// This was a zome call made remotely but
    /// something has failed on the network
    NetworkError(String),
}
