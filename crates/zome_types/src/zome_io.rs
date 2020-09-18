use holochain_serialized_bytes::prelude::*;

use crate::element::ElementVec;

/// All wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way.
macro_rules! wasm_io_types {
    ( $( pub struct $t:ident($t_inner:ty $(,)?); )* ) => {
        $(
            #[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
            pub struct $t($t_inner);

            impl $t {
                pub fn new(i: $t_inner) -> Self {
                    Self(i)
                }

                pub fn into_inner(self) -> $t_inner {
                    self.0
                }

                pub fn inner_ref(&self) -> &$t_inner {
                    &self.0
                }
            }
        )*
    }
}

wasm_io_types!(
    // The zome and agent info are constants specific to the current zome and chain.
    // All the information is provided by core so there is no input value.
    // These are constant for the lifetime of a zome call.
    pub struct ZomeInfoInput(());
    pub struct ZomeInfoOutput(crate::zome_info::ZomeInfo);
    pub struct AgentInfoInput(());
    pub struct AgentInfoOutput(crate::agent_info::AgentInfo);
    // @todo Call is arbitrary so we need to send and receive SerializedBytes.
    pub struct CallInput(SerializedBytes);
    pub struct CallOutput(SerializedBytes);
    // @todo List all the local capability claims.
    pub struct CapabilityClaimsInput(());
    pub struct CapabilityClaimsOutput(());
    // @todo List all the local capability grants.
    pub struct CapabilityGrantsInput(());
    pub struct CapabilityGrantsOutput(());
    // @todo Get the capability for the current zome call.
    pub struct CapabilityInfoInput(());
    pub struct CapabilityInfoOutput(());
    // The EntryDefId determines how a create is handled on the host side.
    // CapGrant and CapClaim are handled natively.
    // App entries are referenced by entry defs then SerializedBytes stuffed into an Entry::App.
    pub struct CreateInput((crate::entry_def::EntryDefId, crate::entry::Entry));
    // Header hash of the newly created element.
    pub struct CreateOutput(holo_hash::HeaderHash);
    // @todo
    pub struct DecryptInput(());
    pub struct DecryptOutput(());
    // @todo
    pub struct EncryptInput(());
    pub struct EncryptOutput(());
    // @todo
    pub struct ShowEnvInput(());
    pub struct ShowEnvOutput(());
    // @todo
    pub struct PropertyInput(());
    pub struct PropertyOutput(());
    // Query the source chain for data.
    pub struct QueryInput(crate::query::ChainQueryFilter);
    pub struct QueryOutput(ElementVec);
    // the length of random bytes to create
    pub struct RandomBytesInput(u32);
    pub struct RandomBytesOutput(crate::bytes::Bytes);
    // Header hash of the CreateLink element.
    pub struct DeleteLinkInput(holo_hash::HeaderHash);
    // Header hash of the DeleteLink element.
    pub struct DeleteLinkOutput(holo_hash::HeaderHash);
    pub struct CallRemoteInput(crate::call_remote::CallRemote);
    pub struct CallRemoteOutput(ZomeCallResponse);
    // @todo
    pub struct SendInput(());
    pub struct SendOutput(());
    // Attempt to have the keystore sign some data
    // The pubkey in the input needs to be found in the keystore for this to work
    pub struct SignInput(crate::signature::SignInput);
    pub struct SignOutput(crate::signature::Signature);
    // @todo
    pub struct ScheduleInput(core::time::Duration);
    pub struct ScheduleOutput(());
    // Same as CreateInput but also takes the HeaderHash of the updated element.
    pub struct UpdateInput(
        (
            crate::entry_def::EntryDefId,
            crate::entry::Entry,
            holo_hash::HeaderHash,
        ),
    );
    // Header hash of the newly committed element.
    pub struct UpdateOutput(holo_hash::HeaderHash);
    // @todo
    pub struct EmitSignalInput(());
    pub struct EmitSignalOutput(());
    // @todo
    pub struct DeleteInput(holo_hash::HeaderHash);
    pub struct DeleteOutput(holo_hash::HeaderHash);
    // Create a link between two entries.
    pub struct CreateLinkInput(
        (
            holo_hash::EntryHash,
            holo_hash::EntryHash,
            crate::link::LinkTag,
        ),
    );
    pub struct CreateLinkOutput(holo_hash::HeaderHash);
    // @todo
    pub struct KeystoreInput(());
    pub struct KeystoreOutput(());
    // Get links by entry hash from the cascade.
    pub struct GetLinksInput((holo_hash::EntryHash, Option<crate::link::LinkTag>));
    pub struct GetLinksOutput(crate::link::Links);
    pub struct GetLinkDetailsInput((holo_hash::EntryHash, Option<crate::link::LinkTag>));
    pub struct GetLinkDetailsOutput(crate::link::LinkDetails);
    // Attempt to get a live entry from the cascade.
    pub struct GetInput((holo_hash::AnyDhtHash, crate::entry::GetOptions));
    pub struct GetOutput(Option<crate::element::Element>);
    pub struct GetDetailsInput((holo_hash::AnyDhtHash, crate::entry::GetOptions));
    pub struct GetDetailsOutput(Option<crate::metadata::Details>);
    // @todo
    pub struct EntryTypePropertiesInput(());
    pub struct EntryTypePropertiesOutput(());
    // Hash an entry on the host.
    pub struct HashEntryInput(crate::entry::Entry);
    pub struct HashEntryOutput(holo_hash::EntryHash);
    // Current system time, in the opinion of the host, as a `Duration`.
    pub struct SysTimeInput(());
    pub struct SysTimeOutput(core::time::Duration);
    // The debug host import takes a DebugMsg to output wherever the host wants to display it.
    // DebugMsg includes line numbers. so the wasm tells the host about it's own code structure.
    pub struct DebugInput(crate::debug::DebugMsg);
    pub struct DebugOutput(());
    // There's nothing to go in or out of a noop.
    // Used to "defuse" host functions when side effects are not allowed.
    pub struct UnreachableInput(());
    pub struct UnreachableOutput(());
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
    pub struct ExternInput(crate::SerializedBytes);
    pub struct ExternOutput(crate::SerializedBytes);
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
}
