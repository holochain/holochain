#[cfg(feature = "build_demo")]
mod demo;
#[cfg(feature = "build_demo")]
pub use demo::*;

#[cfg(feature = "build_integrity_wasm")]
mod integrity_wasm;
#[cfg(feature = "build_integrity_wasm")]
pub use integrity_wasm::*;

#[cfg(feature = "build_coordinator_wasm")]
mod coordinator_wasm;
#[cfg(feature = "build_coordinator_wasm")]
pub use coordinator_wasm::*;

macro_rules! wasm_common {
    () => {
        #[hdk_entry_helper]
        #[derive(Clone)]
        pub struct File {
            pub desc: String,
            pub data: SerializedBytes,
        }

        #[hdk_entry_defs]
        #[unit_enum(UnitEntryTypes)]
        pub enum EntryTypes {
            File(File),
        }

        #[hdk_link_types]
        pub enum LinkTypes {
            AllFiles,
        }
    };
}
pub(crate) use wasm_common;

#[cfg(all(feature = "build_demo", test))]
mod test;
