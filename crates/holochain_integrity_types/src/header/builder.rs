use super::EntryType;
use super::Timestamp;
use crate::header;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::EntryRateWeight;
use crate::HeaderUnweighed;
use crate::HeaderWeighed;
use crate::MembraneProof;
use crate::RateWeight;
use header::Dna;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;

#[derive(Clone, Debug)]
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
pub trait HeaderBuilder<U: HeaderUnweighed>: Sized {
    fn build(self, common: HeaderBuilderCommon) -> U;
}

macro_rules! builder_variant {
    ( $name: ident <$weight : ty> { $($field: ident : $t: ty),* $(,)? } ) => {
        // builder_variant!($name {
        //     $($field: $t),*
        //     |
        //     weight: $weight
        // });

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

        impl HeaderBuilder<header::$name<()>> for $name {
            fn build(self, common: HeaderBuilderCommon) -> header::$name<()> {
                let HeaderBuilderCommon {
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                } = common;

                header::$name {
                    weight: (),
                    author,
                    timestamp,
                    header_seq,
                    prev_header,
                    $($field : self.$field,)*
                }
            }
        }


        impl HeaderWeighed for header::$name {
            type Unweighed = header::$name<()>;
            type Weight = $weight;

            fn into_header(self) -> header::Header {
                header::Header::$name(self)
            }

            fn unweighed(self) -> header::$name<()> {
                header::$name::<()> {
                    weight: (),
                    author: self.author,
                    timestamp: self.timestamp,
                    header_seq: self.header_seq,
                    prev_header: self.prev_header,
                    $($field: self.$field),*
                }
            }
        }

        impl HeaderUnweighed for header::$name<()> {
            type Weighed = header::$name;
            type Weight = $weight;

            fn weighed(self, weight: $weight) -> header::$name {
                header::$name {
                    weight,
                    author: self.author,
                    timestamp: self.timestamp,
                    header_seq: self.header_seq,
                    prev_header: self.prev_header,
                    $($field: self.$field),*
                }
            }
        }


        // impl From<($name, HeaderBuilderCommon)> for header::$name {
        //     fn from((n, h): ($name, HeaderBuilderCommon)) -> header::$name {
        //         n.build(h)
        //     }
        // }

        #[cfg(feature = "test_utils")]
        impl header::$name {
            pub fn from_builder(common: HeaderBuilderCommon, $($field : $t),*) -> Self {
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

        impl HeaderWeighed for header::$name {
            type Unweighed = header::$name;
            type Weight = ();

            fn into_header(self) -> header::Header {
                header::Header::$name(self)
            }

            fn unweighed(self) -> Self::Unweighed {
                self
            }

        }

        impl HeaderUnweighed for header::$name {
            type Weighed = header::$name;
            type Weight = ();

            fn weighed(self, _weight: ()) -> header::$name {
                self
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
                    $($field : self.$field,)*
                    $( $($dfield : self.$dfield),* )?
                }
            }
        }

        impl From<($name, HeaderBuilderCommon)> for header::$name {
            fn from((n, h): ($name, HeaderBuilderCommon)) -> header::$name {
                n.build(h)
            }
        }

        #[cfg(feature = "test_utils")]
        impl header::$name {
            pub fn from_builder(common: HeaderBuilderCommon, $($field : $t),*) -> Self {
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
    link_type: LinkType,
    tag: LinkTag,
});

builder_variant!(DeleteLink {
    link_add_address: HeaderHash,
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
    original_header_address: HeaderHash,

    entry_type: EntryType,
    entry_hash: EntryHash,
});

builder_variant!(Delete<RateWeight> {
    deletes_address: HeaderHash,
    deletes_entry_address: EntryHash,
});

builder_variant!(AgentValidationPkg {
    membrane_proof: Option<MembraneProof>,
});

/// The Dna header can't implement HeaderBuilder because it lacks a
/// `prev_header` field, so this helper is provided as a special case
#[cfg(feature = "test_utils")]
impl Dna {
    pub fn from_builder(hash: DnaHash, builder: HeaderBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            hash,
        }
    }
}

// some more manual implementations for Dna

impl HeaderWeighed for Dna {
    type Unweighed = Dna;
    type Weight = ();

    fn into_header(self) -> header::Header {
        header::Header::Dna(self)
    }

    fn unweighed(self) -> Self::Unweighed {
        self
    }
}

impl HeaderUnweighed for Dna {
    type Weighed = Dna;
    type Weight = ();

    fn weighed(self, _weight: ()) -> Dna {
        self
    }
}
