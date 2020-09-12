use holochain_serialized_bytes::prelude::*;

use crate::header::HeaderHashedVec;

/// all wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way
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
    // the zome and agent info are constants specific to the current zome and state of the source chain
    // all the information is provided by core so there is no input value
    // as these are constant it makes sense for the zome dev or HDK to cache the return of this in
    // a lazy_static! or similar
    pub struct ZomeInfoInput(());
    pub struct ZomeInfoOutput(crate::zome_info::ZomeInfo);
    pub struct AgentInfoInput(());
    pub struct AgentInfoOutput(crate::agent_info::AgentInfo);
    // call is entirely arbitrary so we need to send and receive SerializedBytes
    pub struct CallInput(SerializedBytes);
    pub struct CallOutput(SerializedBytes);
    // list all the local capability claims
    pub struct CapabilityClaimsInput(());
    pub struct CapabilityClaimsOutput(());
    // list all the local capability grants
    pub struct CapabilityGrantsInput(());
    pub struct CapabilityGrantsOutput(());
    // get the capability for the current zome call
    pub struct CapabilityInfoInput(());
    pub struct CapabilityInfoOutput(());
    // the SerializedBytes will be stuffed into an Entry::App(SB) host side
    pub struct CreateEntryInput((crate::entry_def::EntryDefId, crate::entry::Entry));
    // the header hash of the newly created entry
    pub struct CreateEntryOutput(holo_hash::HeaderHash);
    // @TODO
    pub struct DecryptInput(());
    pub struct DecryptOutput(());
    // @TODO
    pub struct EncryptInput(());
    pub struct EncryptOutput(());
    // @TODO
    pub struct ShowEnvInput(());
    pub struct ShowEnvOutput(());
    // @TODO
    pub struct PropertyInput(());
    pub struct PropertyOutput(());
    // Query the source chain for data
    pub struct QueryInput(crate::query::ChainQueryFilter);
    pub struct QueryOutput(HeaderHashedVec);
    // the length of random bytes to create
    pub struct RandomBytesInput(u32);
    pub struct RandomBytesOutput(crate::bytes::Bytes);
    // the header hash of the LinkAdd element
    pub struct RemoveLinkInput(holo_hash::HeaderHash);
    // the header hash of the LinkRemove element
    pub struct RemoveLinkOutput(holo_hash::HeaderHash);
    pub struct CallRemoteInput(crate::call_remote::CallRemote);
    pub struct CallRemoteOutput(ZomeCallInvocationResponse);
    // @TODO
    pub struct SendInput(());
    pub struct SendOutput(());
    // @TODO
    pub struct SignInput(());
    pub struct SignOutput(());
    // @TODO
    pub struct ScheduleInput(core::time::Duration);
    pub struct ScheduleOutput(());
    // Same as CreateEntryInput but also takes the HeaderHash of the entry you are replacing
    pub struct UpdateEntryInput(
        (
            crate::entry_def::EntryDefId,
            crate::entry::Entry,
            holo_hash::HeaderHash,
        ),
    );
    // the header hash of the newly committed entry
    pub struct UpdateEntryOutput(holo_hash::HeaderHash);
    // @TODO
    pub struct EmitSignalInput(());
    pub struct EmitSignalOutput(());
    // @TODO
    pub struct DeleteEntryInput(holo_hash::HeaderHash);
    pub struct DeleteEntryOutput(holo_hash::HeaderHash);
    // create link entries
    pub struct LinkEntriesInput(
        (
            holo_hash::EntryHash,
            holo_hash::EntryHash,
            crate::link::LinkTag,
        ),
    );
    pub struct LinkEntriesOutput(holo_hash::HeaderHash);
    // @TODO
    pub struct KeystoreInput(());
    pub struct KeystoreOutput(());
    // get links from the cascade
    pub struct GetLinksInput((holo_hash::EntryHash, Option<crate::link::LinkTag>));
    pub struct GetLinksOutput(crate::link::Links);
    pub struct GetLinkDetailsInput((holo_hash::EntryHash, Option<crate::link::LinkTag>));
    pub struct GetLinkDetailsOutput(crate::link::LinkDetails);
    // get an entry from the cascade
    pub struct GetInput((holo_hash::AnyDhtHash, crate::entry::GetOptions));
    pub struct GetOutput(Option<crate::element::Element>);
    pub struct GetDetailsInput((holo_hash::AnyDhtHash, crate::entry::GetOptions));
    pub struct GetDetailsOutput(Option<crate::metadata::Details>);
    // @TODO
    pub struct EntryTypePropertiesInput(());
    pub struct EntryTypePropertiesOutput(());
    // hash an entry on the host and get a core hash back
    pub struct EntryHashInput(crate::entry::Entry);
    pub struct EntryHashOutput(holo_hash::EntryHash);
    // the current system time, in the opinion of the host, as a Duration
    pub struct SysTimeInput(());
    pub struct SysTimeOutput(core::time::Duration);
    // the debug host import takes a DebugMsg to output wherever the host wants to display it
    // it is intended that the zome dev or the HDK provides a little sugar to support arbitrary
    // implementations of Debug, e.g. something like a debug! macro that wraps debug_msg! and the
    // host interface
    // DebugMsg includes line numbers etc. so the wasm can tell the host about it's own code
    pub struct DebugInput(crate::debug::DebugMsg);
    pub struct DebugOutput(());
    // there's nothing to go in or out of a noop
    // used to "defuse" host functions when side effects are not allowed
    pub struct UnreachableInput(());
    pub struct UnreachableOutput(());
    // every externed function that the zome developer exposes to holochain returns GuestOutput
    // as the zome developer can expose callbacks in a "sparse" way based on names and the functions
    // can take different input (e.g. validation vs. hooks like init, etc.) all we can say is that
    // some SerializedBytes are being returned
    // in the case of ZomeExtern functions exposed to a client, the data input/output is entirely
    // arbitrary so we can't say anything at all. In this case the happ developer must BYO
    // deserialization context to match the client, either directly or via. the HDK.
    // note though, that _unlike_ zome externs, the host _does_ know exactly the guest should be
    // returning for callbacks, it's just that the unpacking of the return happens in two steps:
    // - first the sparse callback is triggered with SB input/output
    // - then the guest inflates the expected input or the host the expected output based on the
    //   callback flavour
    pub struct HostInput(crate::SerializedBytes);
    pub struct GuestOutput(crate::SerializedBytes);
);

/// Response to a zome invocation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
pub enum ZomeCallInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(GuestOutput),
    /// cap grant failure
    Unauthorized,
}
