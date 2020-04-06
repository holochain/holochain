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
    // the current system time, in the opinion of the host, as a Duration
    [SysTimeInput, ()],
    [SysTimeOutput, core::time::Duration],
    // the debug host import takes a string to output wherever the host wants to display it
    // it is intended that the zome dev or the HDK provides a little sugar to support arbitrary
    // implementations of Debug, e.g. something like a debug! macro that wraps format! and the
    // host interface
    [DebugInput, String],
    [DebugOutput, ()],
    // every externed function that the zome develoepr exposes to holochain returns ZomeExternOutput
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
