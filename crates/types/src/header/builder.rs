use super::EntryType;
use crate::header;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    Header, Timestamp,
};
use derive_more::{Constructor, From};
use holo_hash::*;
use holochain_serialized_bytes::SerializedBytes;

#[derive(Constructor)]
pub struct HeaderBuilderCommon {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
}

/// Builder for non-genesis Headers
///
/// SourceChain::put takes one of these rather than a raw Header, so that it
/// can inject the proper values via `HeaderBuilderCommon`, rather than requiring
/// surrounding code to construct a proper Header outside of the context of
/// the SourceChain.
///
/// This builder does not build pre-genesis Headers, because prior to genesis
/// there is no Agent associated with the source chain, and also the fact that
/// the Dna header has no prev_entry causes a special case that need not be
/// dealt with. SourceChain::genesis already handles genesis in one fell swoop.
#[derive(Clone, Debug, From, PartialEq, Eq)]
pub enum HeaderBuilder {
    InitZomesComplete,
    LinkAdd(LinkAdd),
    LinkRemove(LinkRemove),
    ChainOpen(ChainOpen),
    ChainClose(ChainClose),
    EntryCreate(EntryCreate),
    EntryUpdate(EntryUpdate),
    EntryDelete(EntryDelete),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkAdd {
    pub base_address: AnyDhtHash,
    pub target_address: AnyDhtHash,
    pub tag: SerializedBytes,
    pub link_type: SerializedBytes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRemove {
    pub link_add_address: HeaderAddress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainOpen {
    pub prev_dna_hash: DnaHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainClose {
    pub new_dna_hash: DnaHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryCreate {
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryUpdate {
    pub replaces_address: AnyDhtHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryDelete {
    pub removes_address: AnyDhtHash,
}

impl HeaderBuilder {
    pub fn build(self, common: HeaderBuilderCommon) -> Header {
        let HeaderBuilderCommon {
            author,
            timestamp,
            header_seq,
            prev_header,
        } = common;
        match self {
            HeaderBuilder::InitZomesComplete => header::InitZomesComplete {
                author,
                timestamp,
                header_seq,
                prev_header,
            }
            .into(),

            HeaderBuilder::LinkAdd(LinkAdd {
                base_address,
                target_address,
                tag,
                link_type,
            }) => header::LinkAdd {
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

            HeaderBuilder::LinkRemove(LinkRemove { link_add_address }) => header::LinkRemove {
                author,
                timestamp,
                header_seq,
                prev_header,

                link_add_address,
            }
            .into(),

            HeaderBuilder::ChainOpen(ChainOpen { prev_dna_hash }) => header::ChainOpen {
                author,
                timestamp,
                header_seq,
                prev_header,

                prev_dna_hash,
            }
            .into(),

            HeaderBuilder::ChainClose(ChainClose { new_dna_hash }) => header::ChainClose {
                author,
                timestamp,
                header_seq,
                prev_header,

                new_dna_hash,
            }
            .into(),

            HeaderBuilder::EntryCreate(EntryCreate {
                entry_type,
                entry_hash,
            }) => header::EntryCreate {
                author,
                timestamp,
                header_seq,
                prev_header,

                entry_type,
                entry_hash,
            }
            .into(),

            HeaderBuilder::EntryUpdate(EntryUpdate {
                replaces_address,
                entry_type,
                entry_hash,
            }) => header::EntryUpdate {
                author,
                timestamp,
                header_seq,
                prev_header,

                replaces_address,
                entry_type,
                entry_hash,
            }
            .into(),

            HeaderBuilder::EntryDelete(EntryDelete { removes_address }) => header::EntryDelete {
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
