use super::EntryType;
use crate::header;
use crate::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    Header, Timestamp,
};
use derive_more::Constructor;
use header::HeaderInner;
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
pub trait HeaderBuilder<H: HeaderInner>: Sized {
    fn build_inner(self, common: HeaderBuilderCommon) -> H;
    fn build(self, common: HeaderBuilderCommon) -> H {
        self.build_inner(common)
    }
    fn build_header(self, common: HeaderBuilderCommon) -> Header {
        self.build_inner(common).into()
    }
}

macro_rules! builder_variant {
    ( $name: ident { $($field: ident : $t: ty),* $(,)? } ) => {

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field : $t),*
        }

        impl HeaderBuilder<header::$name> for $name {
            fn build_inner(self, common: HeaderBuilderCommon) -> header::$name {
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
    }
}

builder_variant!(InitZomesComplete {});

builder_variant!(LinkAdd {
    base_address: AnyDhtHash,
    target_address: AnyDhtHash,
    tag: SerializedBytes,
    link_type: SerializedBytes,
});

builder_variant!(LinkRemove {
    link_add_address: HeaderAddress,
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
    replaces_address: AnyDhtHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(EntryDelete {
    removes_address: AnyDhtHash,
});
