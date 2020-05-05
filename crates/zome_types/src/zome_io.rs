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
    // @TODO
    pub struct GetEntryInput(());
    pub struct GetEntryOutput(());
    // @TODO
    pub struct EntryTypePropertiesInput(());
    pub struct EntryTypePropertiesOutput(());
    // @TODO
    pub struct EntryAddressInput(());
    pub struct EntryAddressOutput(());
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
    // every callback function that the zome developer exposes to holochain returns CallbackGuestOutput
    // as the zome developer can expose callbacks in a "sparse" way based on names and the functions
    // can take different input (e.g. validation vs. hooks like init, etc.) all we can say is that
    // some SerializedBytes are being returned
    // note though, that _unlike_ zome externs, the host _does_ know exactly the guest should be
    // returning, it's just that the unpacking of the return happens in two steps:
    // - first the sparse callback is triggered with SB input/output
    // - then the guest inflates the expected input or the host the expected output based on the
    //   callback flavour
    pub struct CallbackHostInput(crate::SerializedBytes);
    pub struct CallbackGuestOutput(crate::SerializedBytes);
    // every externed function that the zome developer exposes to holochain returns ZomeExternOutput
    // as the zome developer can expose arbitrary functions and the client will expect arbitrary data
    // all we can say is that some SerializedBytes are being returned
    // same deal for the input, the HDK or the dev will need to BYO context for deserialization of the
    // inner data
    // IMPORTANT NOTE: zome externs work differently to everything else here because the _host_
    // is providing the input and the _guest_ is providing the output
    // hence, the non-standard naming, to try and make this clear
    pub struct ZomeExternHostInput(crate::SerializedBytes);
    pub struct ZomeExternGuestOutput(crate::SerializedBytes);
);
