use super::EntryType;
use crate::header;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    Header, Timestamp,
};
use holo_hash::*;
use holochain_serialized_bytes::SerializedBytes;

pub struct HeaderCommon {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
}

/// Builder for non-genesis Headers
///
/// SourceChain::put takes one of these rather than a raw Header, so that it
/// can inject the proper values via `HeaderCommon`, rather than requiring
/// surrounding code to construct a proper Header outside of the context of
/// the SourceChain.
///
/// This builder does not build pre-genesis Headers, because prior to genesis
/// there is no Agent associated with the source chain, and also the fact that
/// the Dna header has no prev_entry causes a special case that need not be
/// dealt with. SourceChain::genesis already handles genesis in one fell swoop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderBuilder {
    InitZomesComplete,
    LinkAdd {
        base_address: AnyDhtHash,
        target_address: AnyDhtHash,
        tag: SerializedBytes,
        link_type: SerializedBytes,
    },
    LinkRemove {
        link_add_address: HeaderAddress,
    },
    ChainOpen {
        prev_dna_hash: DnaHash,
    },
    ChainClose {
        new_dna_hash: DnaHash,
    },
    EntryCreate {
        entry_type: EntryType,
        entry_hash: EntryHash,
    },
    EntryUpdate {
        replaces_address: AnyDhtHash,

        entry_type: EntryType,
        entry_hash: EntryHash,
    },
    EntryDelete {
        removes_address: AnyDhtHash,
    },
}

impl HeaderBuilder {
    pub fn build(self, common: HeaderCommon) -> Header {
        use HeaderBuilder::*;
        let HeaderCommon {
            author,
            timestamp,
            header_seq,
            prev_header,
        } = common;
        match self {
            InitZomesComplete => header::InitZomesComplete {
                author,
                timestamp,
                header_seq,
                prev_header,
            }
            .into(),
            LinkAdd {
                base_address,
                target_address,
                tag,
                link_type,
            } => header::LinkAdd {
                author,
                timestamp,
                header_seq,
                prev_header,

                base_address,
                target_address,
                tag,
                link_type,
            }
            .into(),
            LinkRemove { link_add_address } => header::LinkRemove {
                author,
                timestamp,
                header_seq,
                prev_header,

                link_add_address,
            }
            .into(),
            ChainOpen { prev_dna_hash } => header::ChainOpen {
                author,
                timestamp,
                header_seq,
                prev_header,

                prev_dna_hash,
            }
            .into(),
            ChainClose { new_dna_hash } => header::ChainClose {
                author,
                timestamp,
                header_seq,
                prev_header,

                new_dna_hash,
            }
            .into(),
            EntryCreate {
                entry_type,
                entry_hash,
            } => header::EntryCreate {
                author,
                timestamp,
                header_seq,
                prev_header,

                entry_type,
                entry_hash,
            }
            .into(),
            EntryUpdate {
                replaces_address,
                entry_type,
                entry_hash,
            } => header::EntryUpdate {
                author,
                timestamp,
                header_seq,
                prev_header,

                replaces_address,
                entry_type,
                entry_hash,
            }
            .into(),
            EntryDelete { removes_address } => header::EntryDelete {
                author,
                timestamp,
                header_seq,
                prev_header,

                removes_address,
            }
            .into(),
        }
    }
}
