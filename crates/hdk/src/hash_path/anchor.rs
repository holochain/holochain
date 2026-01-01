use super::path::HdkPathExt;
use crate::prelude::*;
use hdi::hash_path::{
    anchor::{Anchor, ROOT},
    path::{Component, Path},
};
use holochain_zome_types::entry::GetStrategy;

/// Extension trait for [`Anchor`] to convert directly to [`TypedPath`] preserving strategy.
pub trait AnchorExt {
    /// Convert this [`Anchor`] to a [`TypedPath`] with the given link type, preserving the anchor's strategy.
    fn to_typed_path<T, E>(&self, link_type: T) -> Result<TypedPath, WasmError>
    where
        ScopedLinkType: TryFrom<T, Error = E>,
        WasmError: From<E>;
}

impl AnchorExt for Anchor {
    fn to_typed_path<T, E>(&self, link_type: T) -> Result<TypedPath, WasmError>
    where
        ScopedLinkType: TryFrom<T, Error = E>,
        WasmError: From<E>,
    {
        let path: Path = self.into();
        let typed_path = path.typed(link_type)?;
        Ok(typed_path.with_strategy(self.strategy))
    }
}

pub trait TryFromPath {
    fn try_from_path(path: &Path) -> Result<Anchor, WasmError>;
}

/// Paths are more general than anchors so a path could be represented that is not a valid anchor.
/// The obvious example would be a path of binary data that is not valid utf-8 strings or a path
/// that is more than 2 levels deep.
impl TryFromPath for Anchor {
    fn try_from_path(path: &Path) -> Result<Self, WasmError> {
        let components: Vec<Component> = path.as_ref().to_owned();
        if components.len() == 2 || components.len() == 3 {
            if components[0] == Component::new(ROOT.to_vec()) {
                Ok(Anchor::new(
                    std::str::from_utf8(components[1].as_ref())
                        .map_err(|e| wasm_error!(SerializedBytesError::Deserialize(e.to_string())))?
                        .to_string(),
                    match components.get(2) {
                        Some(component) => Some(
                            std::str::from_utf8(component.as_ref())
                                .map_err(|e| {
                                    wasm_error!(SerializedBytesError::Deserialize(e.to_string()))
                                })?
                                .to_string(),
                        ),
                        None => None,
                    },
                ))
            } else {
                Err(wasm_error!(WasmErrorInner::Serialize(
                    SerializedBytesError::Deserialize(format!(
                        "Bad anchor path root {:0?} should be {:1?}",
                        components[0].as_ref(),
                        ROOT,
                    ),)
                )))
            }
        } else {
            Err(wasm_error!(WasmErrorInner::Serialize(
                SerializedBytesError::Deserialize(format!(
                    "Bad anchor path length {}",
                    components.len()
                ),)
            )))
        }
    }
}

/// Simple string interface to simple string based paths.
/// a.k.a "the anchor pattern" that predates paths by a few years.
pub fn anchor<T, E>(
    link_type: T,
    anchor_type: String,
    anchor_text: String,
) -> ExternResult<holo_hash::EntryHash>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    anchor_with_strategy(link_type, anchor_type, anchor_text, GetStrategy::default())
}

/// Same as [`anchor`] but allows specifying the [`GetStrategy`].
pub fn anchor_with_strategy<T, E>(
    link_type: T,
    anchor_type: String,
    anchor_text: String,
    strategy: GetStrategy,
) -> ExternResult<holo_hash::EntryHash>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    let anchor = Anchor::new(anchor_type, Some(anchor_text)).with_strategy(strategy);
    let path = anchor.to_typed_path(link_type)?;
    path.ensure()?;
    path.path_entry_hash()
}

/// Returns every hash in a vector from the root of an anchor.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_type_addresses<T, E>(link_type: T) -> ExternResult<Vec<AnyLinkableHash>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    list_anchor_type_addresses_with_strategy(link_type, GetStrategy::default())
}

/// Same as [`list_anchor_type_addresses`] but allows specifying the [`GetStrategy`].
pub fn list_anchor_type_addresses_with_strategy<T, E>(
    link_type: T,
    strategy: GetStrategy,
) -> ExternResult<Vec<AnyLinkableHash>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    let links = Path::from(vec![Component::new(ROOT.to_vec())])
        .typed(link_type)?
        .with_strategy(strategy)
        .children()?
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// Returns every hash in a vector from the second level of an anchor.
/// Uses the string argument to build the path from the root.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_addresses<T, E>(
    link_type: T,
    anchor_type: String,
) -> ExternResult<Vec<AnyLinkableHash>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    list_anchor_addresses_with_strategy(link_type, anchor_type, GetStrategy::default())
}

/// Same as [`list_anchor_addresses`] but allows specifying the [`GetStrategy`].
pub fn list_anchor_addresses_with_strategy<T, E>(
    link_type: T,
    anchor_type: String,
    strategy: GetStrategy,
) -> ExternResult<Vec<AnyLinkableHash>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    let anchor = Anchor::new(anchor_type, None).with_strategy(strategy);
    let links = anchor
        .to_typed_path(link_type)?
        .children()?
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// Old version of holochain that anchors was designed for had two part link tags but now link
/// tags are a single array of bytes, so to get an external interface that is somewhat backwards
/// compatible we need to rebuild the anchors from the paths serialized into the links and then
/// return them.
pub fn list_anchor_tags<T, E>(link_type: T, anchor_type: String) -> ExternResult<Vec<String>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    list_anchor_tags_with_strategy(link_type, anchor_type, GetStrategy::default())
}

/// Same as [`list_anchor_tags`] but allows specifying the [`GetStrategy`].
pub fn list_anchor_tags_with_strategy<T, E>(
    link_type: T,
    anchor_type: String,
    strategy: GetStrategy,
) -> ExternResult<Vec<String>>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    let anchor = Anchor::new(anchor_type, None).with_strategy(strategy);
    let path = anchor.to_typed_path(link_type)?;
    path.ensure()?;
    let hopefully_anchor_tags: Result<Vec<String>, WasmError> = path
        .children_paths()?
        .into_iter()
        .map(|path| match Anchor::try_from_path(&path.path) {
            Ok(anchor) => match anchor.anchor_text {
                Some(text) => Ok(text),
                None => Err(wasm_error!(WasmErrorInner::Serialize(
                    SerializedBytesError::Deserialize("missing anchor text".into(),)
                ))),
            },
            Err(e) => Err(e),
        })
        .collect();
    let mut anchor_tags = hopefully_anchor_tags?;
    anchor_tags.sort();
    anchor_tags.dedup();
    Ok(anchor_tags)
}

#[cfg(test)]
#[test]
fn hash_path_anchor_from_path() {
    let path = Path::from(vec![
        Component::from(vec![0, 0]),
        Component::from(vec![102, 111, 111]),
        Component::from(vec![98, 97, 114]),
    ]);

    assert_eq!(
        Anchor::try_from_path(&path).unwrap(),
        Anchor::new("foo".into(), Some("bar".into())),
    );
}

#[cfg(test)]
#[test]
fn test_anchor_ext_preserves_strategy() {
    // Mock link type for testing
    #[derive(Clone, Copy, Debug)]
    struct TestLinkType;

    impl TryFrom<TestLinkType> for ScopedLinkType {
        type Error = WasmError;

        fn try_from(_: TestLinkType) -> Result<Self, Self::Error> {
            Ok(ScopedLinkType {
                zome_index: 0.into(),
                zome_type: 0.into(),
            })
        }
    }

    // Create an anchor with Local strategy
    let anchor = Anchor::new("test_type".to_string(), Some("test_text".to_string()))
        .with_strategy(GetStrategy::Local);

    // Convert to TypedPath using the extension trait
    let typed_path = anchor
        .to_typed_path(TestLinkType)
        .expect("Should convert to TypedPath");

    // Verify the strategy was preserved
    assert_eq!(typed_path.strategy, GetStrategy::Local);
}
