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

/// Placeholders to allow other things to compile
#[allow(missing_docs)]
pub mod shims;

pub mod time;
pub mod universal_map;

// #[cfg(test)]
pub mod test_utils;

use address::HeaderAddress;
use prelude::*;

/// ChainHeader contains variants for each type of header.
///
/// This struct really defines a local source chain, in the sense that it
/// implements the pointers between hashes that a hash chain relies on, which
/// are then used to check the integrity of data using cryptographic hash
/// functions.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[serde(tag = "type")]
pub enum ChainHeader {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(header::Dna),
    LinkAdd(header::LinkAdd),
    LinkRemove(header::LinkRemove),
    ChainOpen(header::ChainOpen),
    ChainClose(header::ChainClose),
    EntryCreate(header::EntryCreate),
    EntryUpdate(header::EntryUpdate),
    EntryDelete(header::EntryDelete),
}

impl ChainHeader {
    /// Returns `false` if this header is associated with a private entry. Otherwise, returns `true`.
    pub fn is_public(&self) -> bool {
        unimplemented!()
    }

    /// Returns the public key of the agent who signed this header.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            ChainHeader::Dna(i) => &i.author,
            ChainHeader::LinkAdd(i) => &i.author,
            ChainHeader::LinkRemove(i) => &i.author,
            ChainHeader::ChainOpen(i) => &i.author,
            ChainHeader::ChainClose(i) => &i.author,
            ChainHeader::EntryCreate(i) => &i.author,
            ChainHeader::EntryUpdate(i) => &i.author,
            ChainHeader::EntryDelete(i) => &i.author,
        }
    }

    /// returns the timestamp of when the header was created
    pub fn timestamp(&self) -> header::Timestamp {
        unimplemented!()
    }

    // FIXME: use async with_data, or consider wrapper type
    // https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r413226841
    /// Computes the hash of this header.
    pub fn hash(&self) -> HeaderHash {
        // hash the header enum variant struct
        let sb: SerializedBytes = self.try_into().expect("TODO: can this fail?");
        HeaderHash::with_data_sync(&sb.bytes())
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderAddress> {
        Some(match self {
            Self::Dna(header::Dna { .. }) => return None,
            Self::LinkAdd(header::LinkAdd { prev_header, .. }) => prev_header,
            Self::LinkRemove(header::LinkRemove { prev_header, .. }) => prev_header,
            Self::EntryDelete(header::EntryDelete { prev_header, .. }) => prev_header,
            Self::ChainClose(header::ChainClose { prev_header, .. }) => prev_header,
            Self::ChainOpen(header::ChainOpen { prev_header, .. }) => prev_header,
            Self::EntryCreate(header::EntryCreate { prev_header, .. }) => prev_header,
            Self::EntryUpdate(header::EntryUpdate { prev_header, .. }) => prev_header,
        })
    }
}

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
    crate::entry::Entry,
    EntryHash

    crate::ChainHeader,
    HeaderHash

    crate::dna::wasm::DnaWasm,
    WasmHash
);
