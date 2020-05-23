use super::EntryType;
use crate::header;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    Header, Timestamp,
};
use holo_hash::*;
use holochain_serialized_bytes::SerializedBytes;

pub struct HeaderCommon {
    author: AgentPubKey,
    timestamp: Timestamp,
    header_seq: u32,
    prev_header: HeaderAddress,
}

pub enum HeaderBuilder {
    Dna {
        hash: DnaHash,
    },
    AgentValidationPkg {
        membrane_proof: Option<SerializedBytes>,
    },
    InitZomesComplete {},
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
            Dna { hash } => header::Dna {
                author,
                timestamp,
                header_seq,
                // NB: you're forced to pass in a nonsense prev_header for Dna,
                // and it just gets thrown away
                hash,
            }
            .into(),
            AgentValidationPkg { membrane_proof } => header::AgentValidationPkg {
                author,
                timestamp,
                header_seq,
                prev_header,

                membrane_proof,
            }
            .into(),
            InitZomesComplete {} => header::InitZomesComplete {
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
