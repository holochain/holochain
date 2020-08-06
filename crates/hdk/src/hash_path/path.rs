use crate::hash_path::shard::ShardStrategy;
use crate::hash_path::shard::SHARDEND;
use crate::prelude::*;
use holochain_wasmer_guest::*;
use holochain_zome_types::link::LinkTag;
use std::str::FromStr;

/// allows for "foo.bar.baz" to automatically move to/from ["foo", "bar", "baz"] components
/// technically it's moving each string component in as bytes
/// if this is a problem for you simply built the components yourself
/// @see `impl From<String> for Path` below
pub const DELIMITER: &str = ".";

/// "hdk.path" as utf8 bytes
/// all paths use the same link tag and entry def id
/// different pathing schemes/systems/implementations should namespace themselves by their path
/// components rather than trying to layer different link namespaces over the same path components
/// similarly there is no need to define different entry types for different pathing strategies
/// @todo - revisit whether there is a need/use-case for different link tags or entries
/// @see anchors implementation
pub const NAME: [u8; 8] = [0x68, 0x64, 0x6b, 0x2e, 0x70, 0x61, 0x74, 0x68];

/// each path component is arbitrary bytes to be hashed together in a predictable way when the path
/// is hashed to create something that can be linked and discovered by all DHT participants
#[derive(
    Clone, PartialEq, Debug, Default, serde::Deserialize, serde::Serialize, SerializedBytes,
)]
#[repr(transparent)]
pub struct Component(#[serde(with = "serde_bytes")] Vec<u8>);

/// wrap bytes
impl From<Vec<u8>> for Component {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

/// access bytes
impl AsRef<[u8]> for Component {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// unwrap bytes
impl From<Component> for Vec<u8> {
    fn from(component: Component) -> Self {
        component.0
    }
}

/// build a component from a string
/// for many simple use cases we can construct a path out of a string similar to a URI
/// we represent this using the utf32 bytes rather than the utf8 bytes for the chars in the string
/// which gives us a fixed width encoding for strings, which gives us a clean/easy way to support
/// sharding based on strings by iterating over u32s rather than deciding what to do with variable
/// width u8 or u16 characters
impl From<&str> for Component {
    fn from(s: &str) -> Self {
        let bytes: Vec<u8> = s
            .chars()
            .flat_map(|c| (c as u32).to_le_bytes().to_vec())
            .collect();
        Self::from(bytes)
    }
}
/// alias From<&str>
impl From<&String> for Component {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}
/// alias From<&str>
impl From<String> for Component {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

/// restoring a String from a Component requires Vec<u8> to u32 to utf8 handling
impl TryFrom<&Component> for String {
    type Error = SerializedBytesError;
    fn try_from(component: &Component) -> Result<Self, Self::Error> {
        if component.as_ref().len() % 4 != 0 {
            return Err(SerializedBytesError::FromBytes(format!(
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
                            // convert the build vector into 4 le_bytes for the u32
                            // this is an unwrap because we already check the total length above
                            let le_bytes = build[0..std::mem::size_of::<u32>()].try_into().unwrap();
                            let u = u32::from_le_bytes(le_bytes);
                            match std::char::from_u32(u) {
                                Some(c) => {
                                    chars.push(c);
                                    build = vec![];
                                }
                                None => {
                                    error = Some(Err(SerializedBytesError::FromBytes(format!(
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

/// a Path is a vector of components
/// it represents a single traversal of a tree structure down to some arbitrary point
/// the main intent is that we can recursively walk back up the tree, hashing, committing and
/// linking each sub-path along the way until we reach the root
/// at this point, it is possible to follow DHT links from the root back up the path
/// i.e. the ahead-of-time predictability of the hashes of a given path allows us to travel "up"
/// the tree and the linking functionality of the holochain DHT allows us to travel "down" the tree
/// after at least one DHT participant has followed the path "up"
#[derive(
    Clone, Debug, PartialEq, Default, serde::Deserialize, serde::Serialize, SerializedBytes,
)]
#[repr(transparent)]
pub struct Path(Vec<Component>);

entry_def!(Path EntryDef {
    id: core::str::from_utf8(&NAME).unwrap().into(),
    crdt_type: CrdtType,
    required_validations: RequiredValidations::default(),
    visibility: EntryVisibility::Public,
});

/// wrap components vector
impl From<Vec<Component>> for Path {
    fn from(components: Vec<Component>) -> Self {
        Self(components)
    }
}

/// unwrap components vector
impl From<Path> for Vec<Component> {
    fn from(path: Path) -> Self {
        path.0
    }
}

/// access components vector
impl AsRef<Vec<Component>> for Path {
    fn as_ref(&self) -> &Vec<Component> {
        self.0.as_ref()
    }
}

/// split a string path out into a vector of components
/// this allows us to construct pseudo-URI-path-things as strings
/// it is a simpler scheme than URLs and file paths though
/// leading and trailing slashes are ignored as are duplicate slashes and the empty string leads
/// to a path with zero length (no components)
///
/// e.g. all the following result in the same components as `vec!["foo", "bar"]` (as bytes)
/// - foo.bar
/// - foo.bar.
/// - .foo.bar
/// - .foo.bar.
/// - foo..bar
///
/// there is no normalisation of paths, e.g. to guarantee a specific root component exists, at this
/// layer so there is a risk that there are hash collisions with other data on the DHT network if
/// some disambiguation logic is not included in higher level abstractions.
///
/// this supports sharding strategies from a small inline DSL
/// start each component with <width>:<depth># to get shards out of the string
///
/// e.g.
/// - foo.barbaz => normal path as above ["foo", "barbaz"]
/// - foo.1:3#barbazii => width 1, depth 3, ["foo", "b", "a", "r", "barbazii"]
/// - foo.2:3#barbazii => width 2, depth 3, ["foo", "ba", "rb", "az", "barbazii"]
///
/// note that this all works because the components and sharding for strings maps to fixed-width
/// utf32 bytes under the hood rather than variable width bytes
impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self(
            s.split(DELIMITER)
                .filter(|s| !s.is_empty())
                .flat_map(|s| match ShardStrategy::from_str(s) {
                    // handle a strategy if one is found
                    Ok(strategy) => {
                        let (_strategy, component) = s.split_at(s.find(SHARDEND).unwrap());
                        let component = component.trim_start_matches(SHARDEND);
                        let shard_path = Path::from((&strategy, component));
                        let mut shard_components: Vec<Component> = shard_path.into();
                        shard_components.push(Component::from(component));
                        shard_components
                    }
                    // no strategy just use the component directly
                    Err(_) => vec![Component::from(s)],
                })
                .collect(),
        )
    }
}
/// alias From<&str>
impl From<&String> for Path {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}
/// alias From<&str>
impl From<String> for Path {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl TryFrom<&Path> for LinkTag {
    type Error = SerializedBytesError;
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        // link tag is:
        //
        // - the name of all anchor links to disambiguate against other links
        // - the literal serialized bytes of the path
        //
        // this allows the value of the target to be read/dereferenced straight from the
        // link without needing additional network calls
        let path_bytes: Vec<u8> = UnsafeBytes::from(SerializedBytes::try_from(path)?).into();
        let link_tag_bytes: Vec<u8> = NAME.iter().chain(path_bytes.iter()).cloned().collect();
        Ok(LinkTag::new(link_tag_bytes))
    }
}

impl TryFrom<&LinkTag> for Path {
    type Error = SerializedBytesError;
    fn try_from(link_tag: &LinkTag) -> Result<Self, Self::Error> {
        let sb = SerializedBytes::from(UnsafeBytes::from(link_tag.as_ref()[NAME.len()..].to_vec()));
        Ok(Self::try_from(sb)?)
    }
}

impl Path {
    /// what is the hash for the current Path
    pub fn hash(&self) -> Result<holo_hash::EntryHash, WasmError> {
        Ok(entry_hash!(self)?)
    }

    /// does an entry exist at the hash we expect?
    pub fn exists(&self) -> Result<bool, WasmError> {
        Ok(get!(self.hash()?)?.is_some())
    }

    /// recursively touch this and every parent that doesn't exist yet
    pub fn ensure(&self) -> Result<(), WasmError> {
        if !self.exists()? {
            commit_entry!(self)?;
            match self.parent() {
                Some(parent) => {
                    parent.ensure()?;
                    link_entries!(parent.hash()?, self.hash()?, LinkTag::try_from(self)?)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn parent(&self) -> Option<Path> {
        if self.as_ref().len() > 1 {
            let parent_vec: Vec<Component> = self.as_ref()[0..self.as_ref().len() - 1].to_vec();
            Some(parent_vec.into())
        } else {
            None
        }
    }

    /// touch and list all the links from this anchor to anchors below it
    /// only returns links between anchors, not to other entries that might have their own links
    pub fn children(&self) -> Result<holochain_zome_types::link::Links, WasmError> {
        Self::ensure(&self)?;
        let links = get_links!(self.hash()?, holochain_zome_types::link::LinkTag::new(NAME))?;
        // only need one of each hash to build the tree
        let mut unwrapped: Vec<holochain_zome_types::link::Link> = links.into_inner();
        unwrapped.sort();
        unwrapped.dedup();
        Ok(holochain_zome_types::link::Links::from(unwrapped))
    }

    pub fn children_details(&self) -> Result<holochain_zome_types::link::LinkDetails, WasmError> {
        Self::ensure(&self)?;
        Ok(get_link_details!(
            self.hash()?,
            holochain_zome_types::link::LinkTag::new(NAME)
        )?)
    }
}

#[test]
#[cfg(test)]
fn hash_path_delimiter() {
    assert_eq!(".", DELIMITER,);
}

#[test]
#[cfg(test)]
fn hash_path_linktag() {
    assert_eq!("hdk.path".as_bytes(), NAME);

    let path = Path::from("foo.bar");

    let link_tag = LinkTag::try_from(&path).unwrap();

    assert_eq!(
        &vec![
            104, 100, 107, 46, 112, 97, 116, 104, 146, 196, 12, 102, 0, 0, 0, 111, 0, 0, 0, 111, 0,
            0, 0, 196, 12, 98, 0, 0, 0, 97, 0, 0, 0, 114, 0, 0, 0
        ],
        link_tag.as_ref(),
    );

    assert_eq!(Path::try_from(&link_tag).unwrap(), path,);
}

#[test]
#[cfg(test)]
fn hash_path_component() {
    use ::fixt::prelude::*;

    let bytes: Vec<u8> = U8Fixturator::new(fixt::Unpredictable).take(5).collect();

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
        Err(SerializedBytesError::FromBytes(
            "attempted to create u32s from utf8 bytes of length not a factor of 4: length 1".into()
        )),
    );
    assert_eq!(
        String::try_from(&Component::from(vec![9, 9, 9, 9])),
        Err(SerializedBytesError::FromBytes(
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
            let bytes: Vec<u8> = U8Fixturator::new(fixt::Unpredictable).take(10).collect();
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
