#![deny(warnings)]
#![deny(unsafe_code)]
//! `hc demo-cli` provides a method of experiencing holochain without depending
//! on downstream projects such as launcher, dev hub, or the app store.
//! Run `hc demo-cli help` to get started.

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
        /// Storage structure for an hc demo-cli file.
        #[hdk_entry_helper]
        #[derive(Clone)]
        pub struct File {
            pub desc: String,
            pub data: SerializedBytes,
        }

        /// Entry type enum for hc demo-cli.
        #[hdk_entry_defs]
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
