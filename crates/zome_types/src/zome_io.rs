use holochain_serialized_bytes::prelude::*;

/// all wasm shared I/O types need to share the same basic behaviours to cross the host/guest
/// boundary in a predictable way
macro_rules! wasm_io_types {
    ( $( [ $t:ident, $t_inner:ty ] ),* ) => {
        $(
            #[derive(Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
            pub struct $t($t_inner);

            impl $t {
                pub fn new(i: $t_inner) -> Self {
                    Self(i)
                }

                pub fn inner(self) -> $t_inner {
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
    [GlobalsInput, ()],
    [GlobalsOutput, crate::globals::ZomeGlobals],
    // call is entirely arbitrary so we need to send and receive SerializedBytes
    [CallInput, SerializedBytes],
    [CallOutput, SerializedBytes],
    // @TODO
    [CapabilityInput, ()],
    [CapabilityOutput, ()],
    // @TODO
    [CommitEntryInput, ()],
    [CommitEntryOutput, ()],
    // @TODO
    [DecryptInput, ()],
    [DecryptOutput, ()],
    // @TODO
    [EncryptInput, ()],
    [EncryptOutput, ()],
    // @TODO
    [ShowEnvInput, ()],
    [ShowEnvOutput, ()],
    // @TODO
    [PropertyInput, ()],
    [PropertyOutput, ()],
    // @TODO
    [QueryInput, ()],
    [QueryOutput, ()],
    // @TODO
    [RemoveLinkInput, ()],
    [RemoveLinkOutput, ()],
    // @TODO
    [SendInput, ()],
    [SendOutput, ()],
    // @TODO
    [SignInput, ()],
    [SignOutput, ()],
    // @TODO
    [ScheduleInput, core::time::Duration],
    [ScheduleOutput, ()],
    // @TODO
    [UpdateEntryInput, ()],
    [UpdateEntryOutput, ()],
    // @TODO
    [EmitSignalInput, ()],
    [EmitSignalOutput, ()],
    // @TODO
    [RemoveEntryInput, ()],
    [RemoveEntryOutput, ()],
    // @TODO
    [LinkEntriesInput, ()],
    [LinkEntriesOutput, ()],
    // @TODO
    [KeystoreInput, ()],
    [KeystoreOutput, ()],
    // @TODO
    [GetLinksInput, ()],
    [GetLinksOutput, ()],
    // @TODO
    [GetEntryInput, ()],
    [GetEntryOutput, ()],
    // @TODO
    [EntryTypePropertiesInput, ()],
    [EntryTypePropertiesOutput, ()],
    // @TODO
    [EntryAddressInput, ()],
    [EntryAddressOutput, ()],
    // the current system time, in the opinion of the host, as a Duration
    [SysTimeInput, ()],
    [SysTimeOutput, core::time::Duration],
    // the debug host import takes a DebugMsg to output wherever the host wants to display it
    // it is intended that the zome dev or the HDK provides a little sugar to support arbitrary
    // implementations of Debug, e.g. something like a debug! macro that wraps debug_msg! and the
    // host interface
    // DebugMsg includes line numbers etc. so the wasm can tell the host about it's own code
    [DebugInput, crate::debug::DebugMsg],
    [DebugOutput, ()],
    // every externed function that the zome developer exposes to holochain returns ZomeExternOutput
    // as the zome developer can expose arbitrary functions and the client will expect arbitrary data
    // all we can say is that some SerializedBytes are being returned
    // same deal for the input, the HDK or the dev will need to BYO context for deserialization of the
    // inner data
    // IMPORTANT NOTE: zome externs work differently to everything else here because the _host_
    // is providing the input and the _guest_ is providing the output
    // hence, the non-standard naming, to try and make this clear
    [ZomeExternHostInput, crate::SerializedBytes],
    [ZomeExternGuestOutput, crate::SerializedBytes]
);
