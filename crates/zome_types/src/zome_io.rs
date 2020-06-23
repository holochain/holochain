use holochain_serialized_bytes::prelude::*;

/// all wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way
macro_rules! wasm_io_types {
    ( $( pub struct $t:ident($t_inner:ty); )* ) => {
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
    // the globals are constants specific to the current zome and state of the source chain
    // all the information is provided by core so there is no input value
    // as these are constant it makes sense for the zome dev or HDK to cache the return of this in
    // a lazy_static! or similar
    pub struct GlobalsInput(());
    pub struct GlobalsOutput(crate::globals::ZomeGlobals);
    // call is entirely arbitrary so we need to send and receive SerializedBytes
    pub struct CallInput(SerializedBytes);
    pub struct CallOutput(SerializedBytes);
    // @TODO
    pub struct CapabilityInput(());
    pub struct CapabilityOutput(());
    // the SerializedBytes will be stuffed into an Entry::App(SB) host side
    pub struct CommitEntryInput(crate::entry::Entry);
    pub struct CommitEntryOutput(crate::commit::CommitEntryResult);
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
    // @TODO
    pub struct QueryInput(());
    pub struct QueryOutput(());
    // the length of random bytes to create
    pub struct RandomBytesInput(u32);
    pub struct RandomBytesOutput(crate::bytes::Bytes);
    // @TODO
    pub struct RemoveLinkInput(());
    pub struct RemoveLinkOutput(());
    // @TODO
    pub struct SendInput(());
    pub struct SendOutput(());
    // @TODO
    pub struct SignInput(());
    pub struct SignOutput(());
    // @TODO
    pub struct ScheduleInput(core::time::Duration);
    pub struct ScheduleOutput(());
    // @TODO
    pub struct UpdateEntryInput(());
    pub struct UpdateEntryOutput(());
    // @TODO
    pub struct EmitSignalInput(());
    pub struct EmitSignalOutput(());
    // @TODO
    pub struct RemoveEntryInput(());
    pub struct RemoveEntryOutput(());
    // @TODO
    pub struct LinkEntriesInput(());
    pub struct LinkEntriesOutput(());
    // @TODO
    pub struct KeystoreInput(());
    pub struct KeystoreOutput(());
    // @TODO
    pub struct GetLinksInput(());
    pub struct GetLinksOutput(());
    // get an entry from the cascade
    pub struct GetEntryInput((holo_hash_core::HoloHashCore, crate::entry::GetOptions));
    pub struct GetEntryOutput(Option<crate::entry::Entry>);
    // @TODO
    pub struct EntryTypePropertiesInput(());
    pub struct EntryTypePropertiesOutput(());
    // hash an entry on the host and get a core hash back
    pub struct EntryHashInput(crate::entry::Entry);
    pub struct EntryHashOutput(holo_hash_core::HoloHashCore);
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
