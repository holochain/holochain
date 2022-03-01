use crate::hash_path::shard::ShardStrategy;
use crate::hash_path::shard::SHARDEND;
use crate::prelude::*;
use holochain_wasmer_guest::*;
use holochain_zome_types::link::LinkTag;
use holochain_zome_types::validate::RequiredValidationType;
use std::str::FromStr;

/// Allows for "foo.bar.baz" to automatically move to/from ["foo", "bar", "baz"] components.
/// Technically it's moving each string component in as bytes.
/// If this is a problem for you simply build the components yourself as a Vec<Vec<u8>>.
///
/// See `impl From<String> for Path` below.
pub const DELIMITER: &str = ".";

/// Each path component is arbitrary bytes to be hashed together in a predictable way when the path
/// is hashed to create something that can be linked and discovered by all DHT participants.
#[derive(
    Clone, PartialEq, Debug, Default, serde::Deserialize, serde::Serialize, SerializedBytes,
)]
#[repr(transparent)]
pub struct Component(#[serde(with = "serde_bytes")] Vec<u8>);

impl Component {
    pub fn new(v: Vec<u8>) -> Self {
        Self(v)
    }
}

/// Wrap bytes.
impl From<Vec<u8>> for Component {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

/// Access bytes.
impl AsRef<[u8]> for Component {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// Unwrap bytes.
impl From<Component> for Vec<u8> {
    fn from(component: Component) -> Self {
        component.0
    }
}

/// Build a component from a String.
///
/// For many simple use cases we can construct a path out of a string similar to a URI.
/// We represent this using the utf32 bytes rather than the utf8 bytes for the chars in the string
/// which gives us a fixed width encoding for strings, which gives us a clean/easy way to support
/// sharding based on strings by iterating over u32s rather than deciding what to do with variable
/// width u8 or u16 characters.
///
/// IMPORTANT: if you are not using sharding and make heavy use of `Path` then
/// consider building your `Component` directly from `my_string.as_bytes()` to
/// achieve much more compact utf8 representations of each `Component`.
impl From<&str> for Component {
    fn from(s: &str) -> Self {
        let bytes: Vec<u8> = s
            .chars()
            .flat_map(|c| (c as u32).to_le_bytes().to_vec())
            .collect();
        Self::from(bytes)
    }
}
/// Alias From<&str>
impl From<&String> for Component {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}
/// Alias From<&str>
impl From<String> for Component {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

/// Restoring a [ `String` ] from a [ `Component` ] requires [ `Vec<u8>` ] to [ `u32` ] to utf8 handling.
impl TryFrom<&Component> for String {
    type Error = SerializedBytesError;
    fn try_from(component: &Component) -> Result<Self, Self::Error> {
        if component.as_ref().len() % 4 != 0 {
            return Err(SerializedBytesError::Deserialize(format!(
                "attempted to create u32s from utf8 bytes of length not a factor of 4: length {}",
                component.as_ref().len()
            )));
        }
        let (chars, _, error) = component
            .as_ref()
            .iter()
            // @todo this algo seems a bit inefficient but also i'm not sure how much that
            // matters in reality, maybe a premature optimisation to do anything else
            .fold(
                (vec![], vec![], None),
                |(mut chars, mut build, mut error), b| {
                    if error.is_none() {
                        build.push(*b);
                        if build.len() == std::mem::size_of::<u32>() {
                            // Convert the build vector into 4 le_bytes for the u32.
                            // This is an unwrap because we already check the total length above.
                            let le_bytes = build[0..std::mem::size_of::<u32>()].try_into().unwrap();
                            let u = u32::from_le_bytes(le_bytes);
                            match std::char::from_u32(u) {
                                Some(c) => {
                                    chars.push(c);
                                    build = vec![];
                                }
                                None => {
                                    error = Some(Err(SerializedBytesError::Deserialize(format!(
                                        "unknown char for u32: {}",
                                        u
                                    ))));
                                }
                            }
                        }
                    }
                    (chars, build, error)
                },
            );
        match error {
            Some(error) => error,
            None => Ok(chars.iter().collect::<String>()),
        }
    }
}

/// A [ `Path` ] is a vector of [ `Component` ].
/// It represents a single traversal of a tree structure down to some arbitrary point.
/// The main intent is that we can recursively walk back up the tree, hashing, committing and
/// linking each sub-path along the way until we reach the root.
/// At this point it is possible to follow DHT links from the root back up the path.
/// i.e. the ahead-of-time predictability of the hashes of a given path allows us to travel "up"
/// the tree and the linking functionality of the holochain DHT allows us to travel "down" the tree
/// after at least one DHT participant has followed the path "up".
/// Note that the `Path` is not literally committed and/or linked from/to as
/// base and target. For this the [ `PathEntry` ] exists, which achieves a
/// constant size for the DHT representation of each node of the `Path`.
#[derive(
    Clone, Debug, PartialEq, Default, serde::Deserialize, serde::Serialize, SerializedBytes,
)]
#[repr(transparent)]
pub struct Path(Vec<Component>);

entry_def!(Path EntryDef {
    id: "hdk.path".into(),
    crdt_type: CrdtType,
    required_validations: RequiredValidations::default(),
    visibility: EntryVisibility::Public,
    required_validation_type: RequiredValidationType::default(),
});

/// A [ `PathEntry` ] is the hash of a [ `Path` ].
/// This is what is committed and shared on the DHT to build links off as their
/// base and target. If we committed the `Path` directly then the size of each
/// node entry content would be the size of all the components of the path.
/// Given that `ensure` populates all the ancestor nodes committing [ A, B, C ]
/// would create entries with content [ A ], [ A, B ], [ A, B, C ]. For deep
/// paths, or paths with a large component (in bytes) at any node, this would
/// create a lot of redundant data in every descendent entry.
/// Instead, we commit a `PathEntry` so each node is constant size, just the
/// hash of the full `Path` up to that point. This means that committing
/// `PathEntry` instead of `Path` for `[A, B, C]` results in entries with
/// content [ HashA ], [ HashAB ], [ HashABC ]. Note that if A + B + C is much
/// less than the size of a holochain hash (~40 bytes) then this approach is
/// worse than simply committing the `Path` but in practise this is often not
/// the case, and `PathEntry` becomes a more scalable generalised solution.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize, SerializedBytes)]
pub struct PathEntry(#[serde(with = "serde_bytes")] Vec<u8>);

impl PathEntry {
    pub fn new(entry_hash: EntryHash) -> Self {
        Self(entry_hash.get_raw_32().to_vec())
    }
}

entry_def!(PathEntry EntryDef {
    id: "hdk.path_entry".into(),
    crdt_type: CrdtType,
    required_validations: RequiredValidations::default(),
    visibility: EntryVisibility::Public,
    required_validation_type: RequiredValidationType::default(),
});

/// Wrap components vector.
impl From<Vec<Component>> for Path {
    fn from(components: Vec<Component>) -> Self {
        Self(components)
    }
}

/// Unwrap components vector.
impl From<Path> for Vec<Component> {
    fn from(path: Path) -> Self {
        path.0
    }
}

/// Access components vector.
impl AsRef<Vec<Component>> for Path {
    fn as_ref(&self) -> &Vec<Component> {
        self.0.as_ref()
    }
}

/// Split a string path out into a vector of components.
/// This allows us to construct pseudo-URI-path-things as strings.
/// It is a simpler scheme than URLs and file paths.
/// Leading and trailing slashes are ignored as are duplicate dots and the empty string leads
/// to a path with zero length (no components).
///
/// e.g. all the following result in the same components as `vec!["foo", "bar"]` (as bytes)
/// - foo.bar
/// - foo.bar.
/// - .foo.bar
/// - .foo.bar.
/// - foo..bar
///
/// There is no normalisation of paths, e.g. to guarantee a specific root component exists, at this
/// layer so there is a risk that there are hash collisions with other data on the DHT network if
/// some disambiguation logic is not included in higher level abstractions.
///
/// This supports sharding strategies from a small inline DSL.
/// Start each component with <width>:<depth># to get shards out of the string.
///
/// e.g.
/// - foo.barbaz => normal path as above ["foo", "barbaz"]
/// - foo.1:3#barbazii => width 1, depth 3, ["foo", "b", "a", "r", "barbazii"]
/// - foo.2:3#barbazii => width 2, depth 3, ["foo", "ba", "rb", "az", "barbazii"]
///
/// Note that this all works because the components and sharding for strings maps to fixed-width
/// utf32 bytes under the hood rather than variable width bytes.
impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self(
            s.split(DELIMITER)
                .filter(|s| !s.is_empty())
                .flat_map(|s| match ShardStrategy::from_str(s) {
                    // Handle a strategy if one is found.
                    Ok(strategy) => {
                        let (_strategy, component) = s.split_at(s.find(SHARDEND).unwrap());
                        let component = component.trim_start_matches(SHARDEND);
                        let shard_path = Path::from((&strategy, component));
                        let mut shard_components: Vec<Component> = shard_path.into();
                        shard_components.push(Component::from(component));
                        shard_components
                    }
                    // No strategy. Use the component directly.
                    Err(_) => vec![Component::from(s)],
                })
                .collect(),
        )
    }
}
/// Alias From<&str>
impl From<&String> for Path {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}
/// Alias From<&str>
impl From<String> for Path {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl Path {
    pub fn path_entry(&self) -> ExternResult<PathEntry> {
        Ok(PathEntry::new(hash_entry(self)?))
    }

    /// What is the hash for the current [ `Path` ]?
    pub fn path_entry_hash(&self) -> ExternResult<holo_hash::EntryHash> {
        hash_entry(self.path_entry()?)
    }

    /// Does an entry exist at the hash we expect?
    pub fn exists(&self) -> ExternResult<bool> {
        Ok(get(self.path_entry_hash()?, GetOptions::content())?.is_some())
    }

    /// Recursively touch this and every parent that doesn't exist yet.
    pub fn ensure(&self) -> ExternResult<()> {
        if !self.exists()? {
            create_entry(self.path_entry()?)?;
            if let Some(parent) = self.parent() {
                parent.ensure()?;
                create_link(
                    parent.path_entry_hash()?,
                    self.path_entry_hash()?,
                    HdkLinkType::Path,
                    LinkTag::new(match self.leaf() {
                        None => <Vec<u8>>::with_capacity(0),
                        Some(component) => {
                            UnsafeBytes::from(SerializedBytes::try_from(component)?).into()
                        }
                    }),
                )?;
            }
        }
        Ok(())
    }

    /// The parent of the current path is simply the path truncated one level.
    pub fn parent(&self) -> Option<Path> {
        if self.as_ref().len() > 1 {
            let parent_vec: Vec<Component> = self.as_ref()[0..self.as_ref().len() - 1].to_vec();
            Some(parent_vec.into())
        } else {
            None
        }
    }

    /// Touch and list all the links from this path to paths below it.
    /// Only returns links between paths, not to other entries that might have their own links.
    pub fn children(&self) -> ExternResult<Vec<holochain_zome_types::link::Link>> {
        Self::ensure(self)?;
        let mut unwrapped = get_links(self.path_entry_hash()?, None)?;
        // Only need one of each hash to build the tree.
        unwrapped.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));
        unwrapped.dedup_by(|a, b| a.tag.eq(&b.tag));
        Ok(unwrapped)
    }

    /// Touch and list all the links from this path to paths below it.
    /// Same as `Path::children` but returns `Vec<Path>` rather than `Vec<Link>`.
    /// This is more than just a convenience. In general it's not possible to
    /// construct a full `Path` from a child `Link` alone as only a single
    /// `Component` is encoded into the link tag. To build a full child path
    /// the parent path + child link must be combined, which this function does
    /// to produce each child, by using `&self` as that parent.
    pub fn children_paths(&self) -> ExternResult<Vec<Self>> {
        let children = self.children()?;
        let components: ExternResult<Vec<Option<Component>>> = children
            .into_iter()
            .map(|link| {
                let component_bytes = &link.tag.0[1..];
                if component_bytes.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(
                        SerializedBytes::from(UnsafeBytes::from(component_bytes.to_vec()))
                            .try_into()
                            .map_err(WasmError::Serialize)?,
                    ))
                }
            })
            .collect();
        Ok(components?
            .into_iter()
            .map(|maybe_component| {
                let mut new_path = self.clone();
                if let Some(component) = maybe_component {
                    new_path.append_component(component);
                }
                new_path
            })
            .collect())
    }

    pub fn children_details(&self) -> ExternResult<holochain_zome_types::link::LinkDetails> {
        Self::ensure(self)?;
        get_link_details(
            self.path_entry_hash()?,
            Some(holochain_zome_types::link::LinkTag::new([])),
        )
    }

    /// Mutate this `Path` into a child of itself by appending a `Component`.
    pub fn append_component(&mut self, component: Component) {
        self.0.push(component);
    }

    /// Accessor for the last `Component` of this `Path`.
    /// This can be thought of as the leaf of the implied tree structure of
    /// which this `Path` is one branch of.
    pub fn leaf(&self) -> Option<&Component> {
        self.0.last()
    }
}

#[test]
#[cfg(test)]
fn hash_path_delimiter() {
    assert_eq!(".", DELIMITER,);
}

#[test]
#[cfg(test)]
fn hash_path_component() {
    use ::fixt::prelude::*;

    let bytes: Vec<u8> = U8Fixturator::new(Unpredictable).take(5).collect();

    let component = Component::from(bytes.clone());

    assert_eq!(bytes, component.as_ref(),);

    assert_eq!(
        Component::from(vec![102, 0, 0, 0, 111, 0, 0, 0, 111, 0, 0, 0]),
        Component::from("foo"),
    );

    assert_eq!(
        String::try_from(&Component::from(vec![
            102, 0, 0, 0, 111, 0, 0, 0, 111, 0, 0, 0
        ]))
        .unwrap(),
        String::from("foo"),
    );

    assert_eq!(
        String::try_from(&Component::from(vec![1])),
        Err(SerializedBytesError::Deserialize(
            "attempted to create u32s from utf8 bytes of length not a factor of 4: length 1".into()
        )),
    );
    assert_eq!(
        String::try_from(&Component::from(vec![9, 9, 9, 9])),
        Err(SerializedBytesError::Deserialize(
            "unknown char for u32: 151587081".into()
        )),
    );
}

#[test]
#[cfg(test)]
fn hash_path_path() {
    use ::fixt::prelude::*;

    let components: Vec<Component> = {
        let mut vec = vec![];
        for _ in 0..10 {
            let bytes: Vec<u8> = U8Fixturator::new(Unpredictable).take(10).collect();
            vec.push(Component::from(bytes))
        }
        vec
    };

    assert_eq!(&components, Path::from(components.clone()).as_ref(),);

    for (input, output) in vec![
        ("", vec![]),
        (".", vec![]),
        (".foo", vec![Component::from("foo")]),
        ("foo", vec![Component::from("foo")]),
        ("foo.", vec![Component::from("foo")]),
        (".foo.", vec![Component::from("foo")]),
        (
            ".foo.bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            ".foo.bar.",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo.bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo.bar.",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo..bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo.1:3#abcdef",
            vec![
                Component::from("foo"),
                Component::from("a"),
                Component::from("b"),
                Component::from("c"),
                Component::from("abcdef"),
            ],
        ),
        (
            "foo.2:3#zzzzzzzzzz",
            vec![
                Component::from("foo"),
                Component::from("zz"),
                Component::from("zz"),
                Component::from("zz"),
                Component::from("zzzzzzzzzz"),
            ],
        ),
        (
            "foo.1:3#abcdef.bar",
            vec![
                Component::from("foo"),
                Component::from("a"),
                Component::from("b"),
                Component::from("c"),
                Component::from("abcdef"),
                Component::from("bar"),
            ],
        ),
    ] {
        assert_eq!(Path::from(input), Path::from(output),);
    }
}
