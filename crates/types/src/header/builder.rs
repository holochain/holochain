use super::EntryType;
use crate::header;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    Header, Timestamp,
};
use holo_hash::*;
use holochain_serialized_bytes::SerializedBytes;

pub struct HeaderCommon {
    pub author: Option<AgentPubKey>,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: Option<HeaderAddress>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderBuilder {
    // The first three builder variants need to include the AgentPubKey
    // because it has not been committed to the source chain and must come
    // from outside (during genesis)
    // TODO: consider just putting genesis into SourceChainBuf so that
    // SourceChainBuf::put cannot be used to create Genesis headers.
    Dna {
        hash: DnaHash,
        agent_pubkey: AgentPubKey,
    },
    AgentValidationPkg {
        membrane_proof: Option<SerializedBytes>,
        agent_pubkey: AgentPubKey,
    },
    AgentEntry {
        agent_pubkey: AgentPubKey,
    },

    // After this point, we have committed the Agent entry, so we can pull
    // the AgentPubKey from the source chain when building the Header
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

    // bypass builder, just use raw header, for backwards compatibilty
    // with already-written tests and fixtures
    RawHeader(Header),
}

const AUTHOR_MISSING: &'static str = "Must have injected an author into HeaderCommon";
const PREV_HEADER_MISSING: &'static str =
    "Must have injected a prev_header into HeaderCommon for a non-Dna header";

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
            Dna { hash, agent_pubkey } => header::Dna {
                author: agent_pubkey,
                timestamp,
                header_seq,
                // NB: you're forced to pass in a nonsense prev_header for Dna,
                // and it just gets thrown away
                hash,
            }
            .into(),
            AgentValidationPkg {
                membrane_proof,
                agent_pubkey,
            } => header::AgentValidationPkg {
                author: agent_pubkey,
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                membrane_proof,
            }
            .into(),
            AgentEntry { agent_pubkey } => header::EntryCreate {
                author: agent_pubkey.clone(),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                entry_type: EntryType::AgentPubKey,
                entry_hash: agent_pubkey.into(),
            }
            .into(),
            InitZomesComplete {} => header::InitZomesComplete {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),
            }
            .into(),
            LinkAdd {
                base_address,
                target_address,
                tag,
                link_type,
            } => header::LinkAdd {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                base_address,
                target_address,
                tag,
                link_type,
            }
            .into(),
            LinkRemove { link_add_address } => header::LinkRemove {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                link_add_address,
            }
            .into(),
            ChainOpen { prev_dna_hash } => header::ChainOpen {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                prev_dna_hash,
            }
            .into(),
            ChainClose { new_dna_hash } => header::ChainClose {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                new_dna_hash,
            }
            .into(),
            EntryCreate {
                entry_type,
                entry_hash,
            } => header::EntryCreate {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                entry_type,
                entry_hash,
            }
            .into(),
            EntryUpdate {
                replaces_address,
                entry_type,
                entry_hash,
            } => header::EntryUpdate {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                replaces_address,
                entry_type,
                entry_hash,
            }
            .into(),
            EntryDelete { removes_address } => header::EntryDelete {
                author: author.expect(AUTHOR_MISSING),
                timestamp,
                header_seq,
                prev_header: prev_header.expect(PREV_HEADER_MISSING),

                removes_address,
            }
            .into(),

            // bypass builder, just use raw header, for backwards compatibilty
            // with already-written tests and fixtures
            RawHeader(header) => header,
        }
    }
}

impl From<Header> for HeaderBuilder {
    fn from(header: Header) -> HeaderBuilder {
        HeaderBuilder::RawHeader(header)
    }
}
