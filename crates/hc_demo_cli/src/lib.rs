#![deny(warnings)]
#![deny(unsafe_code)]
//! `hc demo-cli` provides a method of experiencing holochain without depending
//! on downstream projects such as launcher, dev hub, or the app store.
//! Run `hc demo-cli help` to get started.

cfg_if::cfg_if! {
    if #[cfg(feature = "build_demo")] {
        mod demo;
        pub use demo::*;
        pub const BUILD_MODE: &str = "build_demo";
    } else if #[cfg(feature = "build_integrity_wasm")] {
        mod integrity_wasm;
        pub use integrity_wasm::*;
        pub const BUILD_MODE: &str = "build_integrity_wasm";
    } else if #[cfg(feature = "build_coordinator_wasm")] {
        mod coordinator_wasm;
        pub use coordinator_wasm::*;
        pub const BUILD_MODE: &str = "build_coordinator_wasm";
    }
}

macro_rules! wasm_common {
    () => {
        /// Storage structure for an hc demo-cli file.
        #[hdk_entry_helper]
        #[derive(Clone)]
        pub struct File {
            pub desc: String,
            pub data: SerializedBytes,
        }

        /// Entry type enum for hc demo-cli.
        #[hdk_entry_types]
        #[unit_enum(UnitEntryTypes)]
        pub enum EntryTypes {
            File(File),
        }

        /// Link type enum for hc demo-cli.
        #[hdk_link_types]
        pub enum LinkTypes {
            AllFiles,
        }
    };
}
pub(crate) use wasm_common;

#[cfg(all(feature = "build_demo", test))]
mod test;
