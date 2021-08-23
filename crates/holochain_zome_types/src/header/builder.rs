use super::EntryType;
use super::Timestamp;
use crate::header;
use crate::header::HeaderInner;
use crate::header::ZomeId;
use crate::link::LinkTag;
use header::Dna;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::SerializedBytes;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct HeaderBuilderCommon {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
}

impl HeaderBuilderCommon {
    pub fn new(
        author: AgentPubKey,
        timestamp: Timestamp,
        header_seq: u32,
        prev_header: HeaderHash,
    ) -> Self {
        Self {
            author,
            timestamp,
            header_seq,
            prev_header,
        }
    }
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
pub trait HeaderBuilder<H: HeaderInner>: Sized {
    fn build(self, common: HeaderBuilderCommon) -> H;
}

macro_rules! builder_variant {
    ( $name: ident { $($field: ident : $t: ty),* $(,)? } ) => {

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field : $t),*
        }

        #[allow(clippy::new_without_default)]
        impl $name {
            pub fn new($($field : $t),* ) -> Self {
                Self {
                    $($field),*
                }
            }
        }

        impl HeaderBuilder<header::$name> for $name {
            fn build(self, common: HeaderBuilderCommon) -> header::$name {
                let HeaderBuilderCommon {
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                } = common;

                header::$name {
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                    $($field : self.$field),*
                }
            }
        }

        impl From<($name, HeaderBuilderCommon)> for header::$name {
            fn from((n, h): ($name, HeaderBuilderCommon)) -> header::$name {
                n.build(h)
            }
        }
        impl header::$name {
            pub fn from_builder(common: HeaderBuilderCommon, $($field : $t),*) -> Self {
                let HeaderBuilderCommon {
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                } = common;

                #[allow(clippy::inconsistent_struct_constructor)]
                header::$name {
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                    $($field),*
                }
            }
        }
    }
}

builder_variant!(InitZomesComplete {});

builder_variant!(CreateLink {
    base_address: EntryHash,
    target_address: EntryHash,
    zome_id: ZomeId,
    tag: LinkTag,
});

builder_variant!(DeleteLink {
    link_add_address: HeaderHash,
    base_address: EntryHash,
});

builder_variant!(OpenChain {
    prev_dna_hash: DnaHash,
});

builder_variant!(CloseChain {
    new_dna_hash: DnaHash,
});

builder_variant!(Create {
    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Update {
    original_entry_address: EntryHash,
    original_header_address: HeaderHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Delete {
    deletes_address: HeaderHash,
    deletes_entry_address: EntryHash,
});

builder_variant!(AgentValidationPkg {
    membrane_proof: Option<SerializedBytes>,
});

impl Dna {
    /// The Dna header can't implement HeaderBuilder because it lacks a
    /// `prev_header` field, so this helper is provided as a special case
    pub fn from_builder(hash: DnaHash, builder: HeaderBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            hash,
        }
    }
}
