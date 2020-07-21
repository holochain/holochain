use super::EntryType;
use crate::header;
use crate::{fixt::*, Timestamp};
use ::fixt::prelude::*;
use derive_more::Constructor;
use header::HeaderInner;
use header::{IntendedFor, ZomeId};
use holo_hash_ext::{fixt::*, *};
use holochain_zome_types::link::LinkTag;

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
pub trait HeaderBuilder<H: HeaderInner>: Sized {
    fn build(self, common: HeaderBuilderCommon) -> H;
}

macro_rules! builder_variant {
    ( $name: ident { $($field: ident : $t: ty),* $(,)? } ) => {

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field : $t),*
        }

        impl $name {
            pub fn new($($field : $t),* ) -> Self {
                Self {
                    $($field),*
                }
            }
        }

        fixturator!(
            $name;
            constructor fn new($($t),*);
        );

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

builder_variant!(LinkAdd {
    base_address: EntryHash,
    target_address: EntryHash,
    zome_id: ZomeId,
    tag: LinkTag,
});

builder_variant!(LinkRemove {
    link_add_address: HeaderHash,
    base_address: EntryHash,
});

builder_variant!(ChainOpen {
    prev_dna_hash: DnaHash,
});

builder_variant!(ChainClose {
    new_dna_hash: DnaHash,
});

builder_variant!(EntryCreate {
    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(EntryUpdate {
    intended_for: IntendedFor,
    replaces_address: HeaderHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(ElementDelete {
    removes_address: HeaderHash,
});

builder_variant!(AgentValidationPkg {
    membrane_proof: MaybeSerializedBytes, // needed for fixturator
});
