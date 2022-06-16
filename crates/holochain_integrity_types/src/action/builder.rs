use super::EntryType;
use super::Timestamp;
use crate::action;
use crate::action::ActionInner;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::MembraneProof;
use action::Dna;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;

#[derive(Clone, Debug)]
pub struct ActionBuilderCommon {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,
}

impl ActionBuilderCommon {
    pub fn new(
        author: AgentPubKey,
        timestamp: Timestamp,
        action_seq: u32,
        prev_action: ActionHash,
    ) -> Self {
        Self {
            author,
            timestamp,
            action_seq,
            prev_action,
        }
    }
}

/// Builder for non-genesis Actions
///
/// SourceChain::put takes one of these rather than a raw Action, so that it
/// can inject the proper values via `ActionBuilderCommon`, rather than requiring
/// surrounding code to construct a proper Action outside of the context of
/// the SourceChain.
///
/// This builder does not build pre-genesis Actions, because prior to genesis
/// there is no Agent associated with the source chain, and also the fact that
/// the Dna action has no prev_entry causes a special case that need not be
/// dealt with. SourceChain::genesis already handles genesis in one fell swoop.
pub trait ActionBuilder<H: ActionInner>: Sized {
    fn build(self, common: ActionBuilderCommon) -> H;
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

        impl ActionBuilder<action::$name> for $name {
            fn build(self, common: ActionBuilderCommon) -> action::$name {
                let ActionBuilderCommon {
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                } = common;

                action::$name {
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                    $($field : self.$field),*
                }
            }
        }

        impl From<($name, ActionBuilderCommon)> for action::$name {
            fn from((n, h): ($name, ActionBuilderCommon)) -> action::$name {
                n.build(h)
            }
        }
        impl action::$name {
            pub fn from_builder(common: ActionBuilderCommon, $($field : $t),*) -> Self {
                let ActionBuilderCommon {
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                } = common;

                #[allow(clippy::inconsistent_struct_constructor)]
                action::$name {
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                    $($field),*
                }
            }
        }
    }
}

builder_variant!(InitZomesComplete {});

builder_variant!(CreateLink {
    base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    link_type: LinkType,
    tag: LinkTag,
});

builder_variant!(DeleteLink {
    link_add_address: ActionHash,
    base_address: AnyLinkableHash,
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
    original_action_address: ActionHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Delete {
    deletes_address: ActionHash,
    deletes_entry_address: EntryHash,
});

builder_variant!(AgentValidationPkg {
    membrane_proof: Option<MembraneProof>,
});

impl Dna {
    /// The Dna action can't implement ActionBuilder because it lacks a
    /// `prev_action` field, so this helper is provided as a special case
    pub fn from_builder(hash: DnaHash, builder: ActionBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            hash,
        }
    }
}
