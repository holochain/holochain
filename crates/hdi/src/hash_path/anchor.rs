use crate::hash_path::path::Component;
use crate::hash_path::path::Path;
use holochain_integrity_types::prelude::GetStrategy;
use holochain_serialized_bytes::prelude::SerializedBytes;

/// This is the root of the [`Path`] tree.
///
/// Forms the entry point to all anchors so that agents can navigate down the tree from here.
pub const ROOT: &[u8; 2] = &[0x00, 0x00];

/// An [`Anchor`] can only be 1 or 2 levels deep as "type" and "text".
///
/// The second level is optional and the Strings use the standard [`TryInto`] for path [`Component`] internally.
///
/// __Anchors are required to be included in an application's [`entry_defs`](crate::prelude::entry_types)__ callback and so implement all the standard methods.
/// Technically the [`Anchor`] entry definition is the [`Path`] definition.
///
/// e.g. `entry_defs![Anchor::entry_def()]`
///
/// The methods implemented on anchor follow the patterns that predate the [`Path`] module but `Path::from(&anchor)` is always possible to use the newer APIs.
#[derive(PartialEq, SerializedBytes, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Anchor {
    pub anchor_type: String,
    pub anchor_text: Option<String>,
    #[serde(default)]
    pub strategy: GetStrategy,
}

impl Anchor {
    /// Create a new [`Anchor`] with the given type and optional text.
    pub fn new(anchor_type: String, anchor_text: Option<String>) -> Self {
        Self {
            anchor_type,
            anchor_text,
            strategy: GetStrategy::default(),
        }
    }

    /// Set the [`GetStrategy`] for this [`Anchor`].
    ///
    /// Note: The strategy is preserved when using HDK anchor functions or the `AnchorExt` trait
    /// to convert to [`TypedPath`](crate::hash_path::path::TypedPath). However, if you convert an
    /// [`Anchor`] to a [`Path`] and then to [`TypedPath`](crate::hash_path::path::TypedPath)
    /// manually, the strategy will be lost since [`Path`] does not carry strategy information.
    pub fn with_strategy(mut self, strategy: GetStrategy) -> Self {
        self.strategy = strategy;
        self
    }
}

/// Anchors are just a special case of path, so we can move from anchor to path losslessly.
/// We simply format the anchor structure into a string that works with the path string handling.
impl From<&Anchor> for Path {
    fn from(anchor: &Anchor) -> Self {
        let mut components = vec![
            Component::new(ROOT.to_vec()),
            Component::from(anchor.anchor_type.as_bytes().to_vec()),
        ];
        if let Some(text) = anchor.anchor_text.as_ref() {
            components.push(Component::from(text.as_bytes().to_vec()));
        }
        components.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_path_root() {
        assert_eq!(ROOT, &[0_u8, 0]);
    }

    #[test]
    fn hash_path_anchor_path() {
        let examples = [
            (
                "foo",
                None,
                Path::from(vec![
                    Component::from(vec![0, 0]),
                    Component::from(vec![102, 111, 111]),
                ]),
            ),
            (
                "foo",
                Some("bar".to_string()),
                Path::from(vec![
                    Component::from(vec![0, 0]),
                    Component::from(vec![102, 111, 111]),
                    Component::from(vec![98, 97, 114]),
                ]),
            ),
        ];
        for (atype, text, path) in examples {
            assert_eq!(path, (&Anchor::new(atype.to_string(), text)).into(),);
        }
    }

    #[test]
    fn test_anchor_with_strategy() {
        use crate::prelude::GetStrategy;
        // Test that Anchor can be created with a specific strategy
        let anchor = Anchor::new("test_type".to_string(), Some("test_text".to_string()));
        assert_eq!(anchor.strategy, GetStrategy::Network); // default

        let anchor_local = anchor.clone().with_strategy(GetStrategy::Local);
        assert_eq!(anchor_local.strategy, GetStrategy::Local);
    }
}
