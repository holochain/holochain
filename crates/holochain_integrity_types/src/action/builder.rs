use super::EntryType;
use super::Timestamp;
use crate::action;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::ActionUnweighed;
use crate::ActionWeighed;
use crate::EntryRateWeight;
use crate::MembraneProof;
use crate::RateWeight;
use crate::ZomeIndex;
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
pub trait ActionBuilder<U: ActionUnweighed>: Sized {
    fn build(self, common: ActionBuilderCommon) -> U;
}

macro_rules! builder_variant {
    ( $name: ident <$weight : ty> { $($field: ident : $t: ty),* $(,)? } ) => {

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field : $t,)*
        }


        #[allow(clippy::new_without_default)]
        impl $name {
            pub fn new($($field : $t),* ) -> Self {
                Self {
                    $($field,)*
                }
            }
        }

        impl ActionBuilder<action::$name<()>> for $name {
            fn build(self, common: ActionBuilderCommon) -> action::$name<()> {
                let ActionBuilderCommon {
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                } = common;

                action::$name {
                    weight: (),
                    author,
                    timestamp,
                    action_seq,
                    prev_action,
                    $($field : self.$field,)*
                }
            }
        }


        impl ActionWeighed for action::$name {
            type Unweighed = action::$name<()>;
            type Weight = $weight;

            fn into_action(self) -> action::Action {
                action::Action::$name(self)
            }

            fn unweighed(self) -> action::$name<()> {
                action::$name::<()> {
                    weight: (),
                    author: self.author,
                    timestamp: self.timestamp,
                    action_seq: self.action_seq,
                    prev_action: self.prev_action,
                    $($field: self.$field),*
                }
            }
        }

        impl ActionUnweighed for action::$name<()> {
            type Weighed = action::$name;
            type Weight = $weight;

            fn weighed(self, weight: $weight) -> action::$name {
                action::$name {
                    weight,
                    author: self.author,
                    timestamp: self.timestamp,
                    action_seq: self.action_seq,
                    prev_action: self.prev_action,
                    $($field: self.$field),*
                }
            }
        }

        #[cfg(feature = "test_utils")]
        impl action::$name {
            pub fn from_builder(common: ActionBuilderCommon, $($field : $t),*) -> Self {
                let builder = $name {
                    $($field,)*
                };

                builder.build(common).weighed(Default::default())
            }
        }
    };

    ( $name: ident { $($field: ident : $t: ty),* $( $(,)? | $($dfield: ident : $dt: ty),* )? $(,)? } ) => {

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field : $t,)*
            $( $(pub $dfield : $dt),* )?
        }

        #[allow(clippy::new_without_default)]
        impl $name {
            pub fn new($($field : $t),* ) -> Self {
                Self {
                    $($field,)*
                    $( $($dfield : Default::default()),* )?
                }
            }

            pub fn new_full($($field : $t,)* $( $($dfield : $dt),* )? ) -> Self {
                Self {
                    $($field,)*
                    $( $($dfield),* )?
                }
            }
        }

        impl ActionWeighed for action::$name {
            type Unweighed = action::$name;
            type Weight = ();

            fn into_action(self) -> action::Action {
                action::Action::$name(self)
            }

            fn unweighed(self) -> Self::Unweighed {
                self
            }

        }

        impl ActionUnweighed for action::$name {
            type Weighed = action::$name;
            type Weight = ();

            fn weighed(self, _weight: ()) -> action::$name {
                self
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
                    $($field : self.$field,)*
                    $( $($dfield : self.$dfield),* )?
                }
            }
        }

        impl From<($name, ActionBuilderCommon)> for action::$name {
            fn from((n, h): ($name, ActionBuilderCommon)) -> action::$name {
                n.build(h)
            }
        }

        #[cfg(feature = "test_utils")]
        impl action::$name {
            pub fn from_builder(common: ActionBuilderCommon, $($field : $t),*) -> Self {
                let builder = $name {
                    $($field,)*
                    $( $($dfield : Default::default()),* )?
                };

                builder.build(common)
            }
        }
    }
}

builder_variant!(InitZomesComplete {});

builder_variant!(CreateLink<RateWeight> {
    base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    zome_index: ZomeIndex,
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

builder_variant!(Create<EntryRateWeight> {
    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Update<EntryRateWeight> {
    original_entry_address: EntryHash,
    original_action_address: ActionHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Delete<RateWeight> {
    deletes_address: ActionHash,
    deletes_entry_address: EntryHash,
});

builder_variant!(AgentValidationPkg {
    membrane_proof: Option<MembraneProof>,
});

/// The Dna action can't implement ActionBuilder because it lacks a
/// `prev_action` field, so this helper is provided as a special case
#[cfg(feature = "test_utils")]
impl Dna {
    pub fn from_builder(hash: DnaHash, builder: ActionBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            hash,
        }
    }
}

// some more manual implementations for Dna

impl ActionWeighed for Dna {
    type Unweighed = Dna;
    type Weight = ();

    fn into_action(self) -> action::Action {
        action::Action::Dna(self)
    }

    fn unweighed(self) -> Self::Unweighed {
        self
    }
}

impl ActionUnweighed for Dna {
    type Weighed = Dna;
    type Weight = ();

    fn weighed(self, _weight: ()) -> Dna {
        self
    }
}
