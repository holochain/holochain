//! Information about the current zome and dna.
use crate::action::ZomeIndex;
use crate::signature::Signature;
use crate::zome::ZomeName;
use crate::DnaModifiers;
use crate::EntryDefIndex;
use crate::EntryDefs;
use crate::FunctionName;
use crate::LinkType;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

#[cfg(test)]
mod test;

/// An optional, app-defined summary describing the "opening" or "closing" state
/// of a cell's source chain.
///
/// An *opening summary* is supplied at app installation time (the install-time
/// analogue of the membrane proof, see `RoleSettings::Provisioned`) and is
/// committed on-chain as the final genesis record, carried by [`Action::OpenChain`].
/// A *closing summary* is supplied when a chain is closed during migration and is
/// carried by [`Action::CloseChain`].
///
/// [`Action::OpenChain`]: crate::action::Action::OpenChain
/// [`Action::CloseChain`]: crate::action::Action::CloseChain
///
/// # Validation is the application's responsibility
///
/// Holochain treats [`ChainSummary::data`] as opaque bytes and does **not**
/// interpret it or verify [`ChainSummary::signatures`] in system validation. The
/// summary record is structurally valid as far as core is concerned as soon as
/// the action it rides on is. **If your application requires the summary to be
/// signed by particular agents, your integrity zome must enforce that itself** —
/// core will not do it for you.
///
/// Each entry in [`ChainSummary::signatures`] is a `(signer, signature)` pair,
/// where `signature` is over the [`ChainSummary::data`] payload by `signer`'s
/// key. The signer's public key travels with the signature, so an integrity zome
/// can verify each one with `verify_signature(signer, signature, data)` (the host
/// function is available in both the `validate` and `genesis_self_check`
/// callbacks):
///
/// ```ignore
/// for (signer, signature) in &summary.signatures {
///     if !verify_signature(signer.clone(), signature.clone(), summary.data.clone())? {
///         return Ok(ValidateCallbackResult::Invalid("bad summary signature".into()));
///     }
/// }
/// // ...then check that `signer`s are the agents your app actually requires.
/// ```
///
/// ## Where to validate (opening summary)
///
/// The opening summary is committed as the final genesis record, and genesis
/// records are integrated *without* local app-validation for their author. That
/// means **the authoring node only runs `genesis_self_check` over its own
/// opening summary, not `validate`** — so an app that wants to reject a bad
/// opening summary *before joining the network* must do so in
/// `genesis_self_check` (which receives it via `GenesisSelfCheckDataV2`).
/// Peers that later receive the record validate it through the integrity zome's
/// `validate` callback (`OpRecord::OpenChain` / `OpActivity::OpenChain`). Put your
/// summary-checking logic in a shared helper and call it from both callbacks.
///
/// The closing summary rides on `CloseChain` and is a normal (non-genesis)
/// record, so it is validated through `validate` on author and peers alike.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq, Hash)]
pub struct ChainSummary {
    /// Opaque, app-defined summary bytes. This is the payload that each entry in
    /// [`ChainSummary::signatures`] signs over.
    pub data: SerializedBytes,
    /// `(signer, signature)` pairs over [`ChainSummary::data`], to be interpreted
    /// and verified by the app.
    pub signatures: Vec<(AgentPubKey, Signature)>,
}

impl ChainSummary {
    /// Construct a new chain summary from opaque payload bytes and a list of
    /// `(signer, signature)` pairs over that payload.
    pub fn new(data: SerializedBytes, signatures: Vec<(AgentPubKey, Signature)>) -> Self {
        Self { data, signatures }
    }
}

#[cfg(test)]
mod chain_summary_tests {
    use super::*;

    #[test]
    fn round_trips_through_serialized_bytes_and_option() {
        let summary = ChainSummary::new(
            UnsafeBytes::from(vec![1u8, 2, 3, 4]).into(),
            vec![
                (AgentPubKey::from_raw_36(vec![1u8; 36]), Signature([7u8; 64])),
                (AgentPubKey::from_raw_36(vec![2u8; 36]), Signature([9u8; 64])),
            ],
        );

        // Round-trip through SerializedBytes (the canonical encoding used for
        // persistence).
        let sb: SerializedBytes = summary.clone().try_into().unwrap();
        let decoded: ChainSummary = sb.try_into().unwrap();
        assert_eq!(summary, decoded);

        // `Option<ChainSummary>` is the type carried by the OpenChain/CloseChain
        // actions.
        let encoded = holochain_serialized_bytes::encode(&Some(summary.clone())).unwrap();
        let decoded: Option<ChainSummary> =
            holochain_serialized_bytes::decode(&encoded).unwrap();
        assert_eq!(Some(summary), decoded);

        let encoded_none = holochain_serialized_bytes::encode(&None::<ChainSummary>).unwrap();
        let decoded_none: Option<ChainSummary> =
            holochain_serialized_bytes::decode(&encoded_none).unwrap();
        assert_eq!(None, decoded_none);
    }
}

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeIndex,
    pub properties: SerializedBytes,
    pub entry_defs: EntryDefs,
    // @todo make this include function signatures when they exist.
    pub extern_fns: Vec<FunctionName>,
    /// All the zome types that are in scope for this zome.
    pub zome_types: ScopedZomeTypesSet,
}

impl ZomeInfo {
    /// Create a new ZomeInfo.
    pub fn new(
        name: ZomeName,
        id: ZomeIndex,
        properties: SerializedBytes,
        entry_defs: EntryDefs,
        extern_fns: Vec<FunctionName>,
        zome_types: ScopedZomeTypesSet,
    ) -> Self {
        Self {
            name,
            id,
            properties,
            entry_defs,
            extern_fns,
            zome_types,
        }
    }
}

/// Placeholder for a real network seed type. See [`DnaModifiers`].
pub type NetworkSeed = String;

/// Information about the current DNA.
#[derive(Debug, Serialize, Deserialize)]
pub struct DnaInfoV1 {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The properties of this DNA.
    pub properties: SerializedBytes,
    // In ZomeIndex order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}

/// Information about the current DNA.
#[derive(Debug, Serialize, Deserialize)]
pub struct DnaInfoV2 {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The modifiers for this DNA.
    pub modifiers: DnaModifiers,
    // In ZomeIndex order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}

/// Convenience alias to the latest `DnaInfoN`.
pub type DnaInfo = DnaInfoV2;

/// The set of [`EntryDefIndex`] and [`LinkType`]s in scope for the calling zome.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
pub struct ScopedZomeTypesSet {
    /// All the entry [`EntryDefIndex`]s in scope for this zome.
    pub entries: ScopedZomeTypes<EntryDefIndex>,
    /// All the entry [`LinkType`]s in scope for this zome.
    pub links: ScopedZomeTypes<LinkType>,
}

/// zome types that are in scope for the calling zome.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopedZomeTypes<T>(pub Vec<(ZomeIndex, Vec<T>)>);

impl<T> Default for ScopedZomeTypes<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

/// A key to the [`ScopedZomeTypes`] container.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZomeTypesKey<T>
where
    T: U8Index + Copy,
{
    /// The index into the [`ZomeIndex`] vec.
    pub zome_index: ZomeDependencyIndex,
    /// The index into the types vec.
    pub type_index: T,
}

/// A key to the [`ScopedZomeTypes<EntryDefIndex>`] container.
pub type ZomeEntryTypesKey = ZomeTypesKey<EntryDefIndex>;
/// A key to the [`ScopedZomeTypes<LinkType>`] container.
pub type ZomeLinkTypesKey = ZomeTypesKey<LinkType>;

/// The index into the [`ZomeIndex`] vec.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZomeDependencyIndex(pub u8);

/// A type with the zome that it is defined in.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopedZomeType<T> {
    /// The zome that defines this type.
    pub zome_index: ZomeIndex,
    /// The type that is defined.
    pub zome_type: T,
}

/// An [`EntryDefIndex`] within the scope of the zome where it's defined.
pub type ScopedEntryDefIndex = ScopedZomeType<EntryDefIndex>;
/// A [`LinkType`] within the scope of the zome where it's defined.
pub type ScopedLinkType = ScopedZomeType<LinkType>;

impl<T> ScopedZomeTypes<T>
where
    T: U8Index + Copy,
{
    /// Get a [`ScopedZomeType`] if one exist at this key.
    pub fn get<K>(&self, key: K) -> Option<ScopedZomeType<T>>
    where
        K: Into<ZomeTypesKey<T>>,
    {
        let key = key.into();
        self.0
            .get(key.zome_index.index())
            .and_then(|(zome_index, types)| {
                types
                    .get(key.type_index.index())
                    .copied()
                    .map(|zome_type| ScopedZomeType {
                        zome_index: *zome_index,
                        zome_type,
                    })
            })
    }

    /// Find the user type in the given iterator that matches this [`ScopedZomeType`].
    pub fn find<I, K>(&self, iter: I, scoped_type: ScopedZomeType<T>) -> Option<I::Item>
    where
        I: IntoIterator<Item = K>,
        K: Into<ZomeTypesKey<T>> + Copy,
        T: PartialEq,
    {
        iter.into_iter()
            .find_map(|key| (self.get(key)? == scoped_type).then_some(key))
    }

    /// Find the [`ZomeTypesKey`] for this [`ScopedZomeType`].
    pub fn find_key(&self, scoped_type: ScopedZomeType<T>) -> Option<ZomeTypesKey<T>>
    where
        T: PartialEq,
        T: From<u8>,
    {
        self.0
            .iter()
            .position(|(zome_index, _)| *zome_index == scoped_type.zome_index)
            .and_then(|zome_index| {
                // Safe to index because we just checked position.
                self.0[zome_index]
                    .1
                    .iter()
                    .position(|zome_type| *zome_type == scoped_type.zome_type)
                    .and_then(|type_index| {
                        Some(ZomeTypesKey {
                            zome_index: u8::try_from(zome_index).ok()?.into(),
                            type_index: u8::try_from(type_index).ok()?.into(),
                        })
                    })
            })
    }

    /// Get all the [`ZomeIndex`] dependencies for the calling zome.
    pub fn dependencies(&self) -> impl Iterator<Item = ZomeIndex> + '_ {
        self.0.iter().map(|(zome_index, _)| *zome_index)
    }
}

impl From<EntryDefIndex> for ZomeEntryTypesKey {
    fn from(type_index: EntryDefIndex) -> Self {
        Self {
            zome_index: 0.into(),
            type_index,
        }
    }
}

impl From<LinkType> for ZomeLinkTypesKey {
    fn from(type_index: LinkType) -> Self {
        Self {
            zome_index: 0.into(),
            type_index,
        }
    }
}

/// This is an internally used trait for checking
/// enum lengths at compile time.
/// This is used by proc macros in the
/// `hdk_derive` crate and should not be used directly.
#[doc(hidden)]
pub trait EnumLen {
    /// The total length of an enum (possibly recusively)
    /// known at compile time.
    const ENUM_LEN: u8;
}

/// This is an internally used trait for checking
/// enum variant lengths at compile time.
/// This is used by proc macros in the
/// `hdk_derive` crate and should not be used directly.
/// `V` is the variant index.
#[doc(hidden)]
pub trait EnumVariantLen<const V: u8> {
    /// The starting point of this enum variant.
    const ENUM_VARIANT_START: u8;
    /// The length of this enum variant.
    /// This could include the recusive length of a nested enum.
    const ENUM_VARIANT_INNER_LEN: u8;
    /// The ending point of this variant.
    const ENUM_VARIANT_LEN: u8 = Self::ENUM_VARIANT_START + Self::ENUM_VARIANT_INNER_LEN;
}

/// Helper trait for types that are internally
/// represented as [`u8`] but need to be used
/// as indicies into containers.
pub trait U8Index {
    /// Get the [`usize`] index from this type.
    fn index(&self) -> usize;
}

impl U8Index for ZomeDependencyIndex {
    fn index(&self) -> usize {
        self.0 as usize
    }
}
impl U8Index for EntryDefIndex {
    fn index(&self) -> usize {
        self.0 as usize
    }
}
impl U8Index for LinkType {
    fn index(&self) -> usize {
        self.0 as usize
    }
}

impl From<u8> for ZomeDependencyIndex {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

impl From<()> for ZomeEntryTypesKey {
    fn from(_: ()) -> Self {
        unimplemented!("Should not ever be used")
    }
}
