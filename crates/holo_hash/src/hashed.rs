use crate::HasHash;
use crate::HashableContent;
use crate::HoloHashOf;
#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

/// Represents some piece of content along with its hash representation, so that
/// hashes need not be calculated multiple times.
/// Provides an easy constructor which consumes the content.
// MAYBE: consider making lazy with OnceCell
#[cfg_attr(feature = "serialization", derive(Debug, Serialize, Deserialize))]
pub struct HoloHashed<C: HashableContent> {
    /// The content which is hashed of type C.
    pub content: C,
    /// The hash of the content C.
    pub hash: HoloHashOf<C>,
}

// `#[derive(ts_rs::TS)]` cannot be used here: `WithoutGenerics` substitutes
// `ts_rs::Dummy` for `C`, but `Dummy` doesn't implement `HashableContent` as
// `HoloHashOf<C>` requires. Hand-written instead, declared as a genuine
// generic type rather than inline-only, so ordinary fields (e.g.
// `Record::signed_action: SignedHashed<Action>`) still pull in the
// `HoloHash` import. `WithoutGenerics` reuses `HoloHashTs` rather than
// `Self`, to avoid leaking a concrete `C` into the shared declaration file.
#[cfg(feature = "ts_rs")]
impl<C> ts_rs::TS for HoloHashed<C>
where
    C: HashableContent + ts_rs::TS,
{
    type WithoutGenerics = crate::ts::HoloHashTs;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        format!("HoloHashed<{}>", C::name(cfg))
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        // Reference `C` by name, not `C::inline(cfg)` — an unadorned field
        // renders as the field type's name, so inlining here would duplicate
        // `C`'s body and break the import `visit_dependencies` registers.
        format!("{{ content: {}, hash: HoloHash }}", C::name(cfg))
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type HoloHashed<C> = { content: C, hash: HoloHash };".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        format!("type HoloHashed = {};", <Self as ts_rs::TS>::inline(cfg))
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<C>();
        C::visit_dependencies(v);
        v.visit::<crate::ts::HoloHashTs>();
    }

    fn visit_generics(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        C::visit_generics(v);
        v.visit::<C>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("types.ts".into())
    }
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_tests {
    use super::*;
    use ts_rs::TS;

    #[derive(ts_rs::TS)]
    #[ts(export_to = "types.ts")]
    struct TestHashedContent {
        #[expect(
            dead_code,
            reason = "field exists only so the TS derive has a non-trivial body to name"
        )]
        value: String,
    }

    impl HashableContent for TestHashedContent {
        type HashType = crate::hash_type::Action;

        fn hash_type(&self) -> Self::HashType {
            crate::hash_type::Action
        }

        fn hashable_content(&self) -> crate::HashableContentBytes {
            crate::HashableContentBytes::Prehashed39(vec![0; 39])
        }
    }

    #[test]
    fn content_field_references_c_by_name_not_inline() {
        let cfg = ts_rs::Config::default();

        let inline = HoloHashed::<TestHashedContent>::inline(&cfg);
        assert_eq!(inline, "{ content: TestHashedContent, hash: HoloHash }");

        let deps = HoloHashed::<TestHashedContent>::dependencies(&cfg);
        assert!(
            deps.iter().any(|dep| dep.ts_name == "TestHashedContent"),
            "expected a dependency on TestHashedContent, got {deps:?}"
        );
    }

    #[test]
    fn name_and_decl_are_a_real_generic_declaration() {
        let cfg = ts_rs::Config::default();

        assert_eq!(
            HoloHashed::<TestHashedContent>::name(&cfg),
            "HoloHashed<TestHashedContent>"
        );
        assert_eq!(
            HoloHashed::<TestHashedContent>::decl(&cfg),
            "type HoloHashed<C> = { content: C, hash: HoloHash };"
        );
    }
}

impl<C: HashableContent> HasHash for HoloHashed<C> {
    type HashType = C::HashType;

    fn as_hash(&self) -> &HoloHashOf<C> {
        &self.hash
    }

    fn into_hash(self) -> HoloHashOf<C> {
        self.hash
    }
}

impl<C> HoloHashed<C>
where
    C: HashableContent,
{
    /// Combine content with its precalculated hash
    pub fn with_pre_hashed(content: C, hash: HoloHashOf<C>) -> Self {
        Self { content, hash }
    }

    // NB: as_hash and into_hash are provided by the HasHash impl

    /// Accessor for content
    pub fn as_content(&self) -> &C {
        &self.content
    }

    /// Mutable accessor for content.
    /// Only useful for heavily mocked/fixturated data in testing.
    /// Guaranteed the hash will no longer match the content if mutated.
    #[cfg(feature = "test_utils")]
    pub fn as_content_mut(&mut self) -> &mut C {
        &mut self.content
    }

    /// Convert to content
    pub fn into_content(self) -> C {
        self.content
    }

    /// Deconstruct as a tuple
    pub fn into_inner(self) -> (C, HoloHashOf<C>) {
        (self.content, self.hash)
    }

    /// Convert to a different content type via From
    #[cfg(feature = "test_utils")]
    pub fn downcast<D>(&self) -> HoloHashed<D>
    where
        C: Clone,
        C::HashType: crate::hash_type::HashTypeSync,
        D: HashableContent<HashType = C::HashType> + From<C>,
    {
        let old_hash = &self.hash;
        let content: D = self.content.clone().into();
        let hashed = HoloHashed::from_content_sync_exact(content);
        assert_eq!(&hashed.hash, old_hash);
        hashed
    }
}

impl<C> Clone for HoloHashed<C>
where
    C: HashableContent + Clone,
{
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            hash: self.hash.clone(),
        }
    }
}

impl<C> std::convert::From<HoloHashed<C>> for (C, HoloHashOf<C>)
where
    C: HashableContent,
{
    fn from(g: HoloHashed<C>) -> (C, HoloHashOf<C>) {
        g.into_inner()
    }
}

impl<C> std::ops::Deref for HoloHashed<C>
where
    C: HashableContent,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.as_content()
    }
}

impl<C> std::convert::AsRef<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn as_ref(&self) -> &C {
        self.as_content()
    }
}

impl<C> std::borrow::Borrow<C> for HoloHashed<C>
where
    C: HashableContent,
{
    fn borrow(&self) -> &C {
        self.as_content()
    }
}

impl<C> std::cmp::PartialEq for HoloHashed<C>
where
    C: HashableContent,
{
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl<C> std::cmp::Eq for HoloHashed<C> where C: HashableContent {}

impl<C> std::hash::Hash for HoloHashed<C>
where
    C: HashableContent,
{
    fn hash<StdH: std::hash::Hasher>(&self, state: &mut StdH) {
        std::hash::Hash::hash(&self.hash, state)
    }
}

impl<C> std::cmp::PartialOrd for HoloHashed<C>
where
    C: HashableContent + PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.content.partial_cmp(&other.content)
    }
}

impl<C> std::cmp::Ord for HoloHashed<C>
where
    C: HashableContent + Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.content.cmp(&other.content)
    }
}

impl<C: HashableContent> HashableContent for HoloHashed<C> {
    type HashType = C::HashType;

    fn hash_type(&self) -> Self::HashType {
        C::hash_type(self)
    }

    fn hashable_content(&self) -> crate::HashableContentBytes {
        crate::HashableContentBytes::Prehashed39(self.as_hash().get_raw_39().to_vec())
    }
}
