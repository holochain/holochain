//! Common holochain types crate.

#![allow(clippy::cognitive_complexity)]
#![deny(missing_docs)]

pub mod address;
pub mod autonomic;
pub mod cell;
pub mod db;
pub mod dna;
pub mod entry;
pub mod header;
pub mod link;
pub mod nucleus;
pub mod observability;
pub mod persistence;
pub mod prelude;
mod timestamp;

/// Placeholders to allow other things to compile
#[allow(missing_docs)]
pub mod shims;

pub mod universal_map;

// #[cfg(test)]
pub mod test_utils;

#[doc(inline)]
pub use header::Header;

pub use timestamp::Timestamp;

use holochain_zome_types;

macro_rules! serial_hash {
    ( $( $input:ty, $output:ident )* ) => {
        $(
            impl std::convert::TryFrom<$input> for holo_hash::$output {
                type Error = holochain_serialized_bytes::SerializedBytesError;
                fn try_from(i: $input) -> Result<Self, Self::Error> {
                    holo_hash::$output::try_from(&i)
                }
            }
            impl std::convert::TryFrom<&$input> for holo_hash::$output {
                type Error = holochain_serialized_bytes::SerializedBytesError;
                fn try_from(i: &$input) -> Result<Self, Self::Error> {
                    Ok(holo_hash::$output::with_data_sync(
                        holochain_serialized_bytes::SerializedBytes::try_from(i)?.bytes(),
                    ))
                }
            }

            impl std::convert::TryFrom<&$input> for holo_hash::HoloHash {
                type Error = holochain_serialized_bytes::SerializedBytesError;
                fn try_from(i: &$input) -> Result<Self, Self::Error> {
                    Ok(holo_hash::HoloHash::$output(holo_hash::$output::try_from(
                        i
                    )?))
                }
            }
            impl std::convert::TryFrom<$input> for holo_hash::HoloHash {
                type Error = holochain_serialized_bytes::SerializedBytesError;
                fn try_from(i: $input) -> Result<Self, Self::Error> {
                    holo_hash::HoloHash::try_from(&i)
                }
            }
        )*
    };
}

serial_hash!(
    crate::dna::wasm::DnaWasm,
    WasmHash
);
