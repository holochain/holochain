use crate::hash_path::shard::ShardStrategy;
use crate::hash_path::shard::SHARDEND;
use crate::prelude::*;
use holochain_wasmer_guest::*;
use std::str::FromStr;

/// allows for "foo/bar/baz" to automatically move to/from ["foo", "bar", "baz"] components
/// technically it's moving each string component in as bytes
/// if this is a problem for you simply built the components yourself
/// @see `impl From<String> for Path` below
pub const DELIMITER: &str = "/";

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

impl TryFrom<&Component> for String {
    type Error = SerializedBytesError;
    fn try_from(component: &Component) -> Result<Self, Self::Error> {
        let (chars, build) =
            component
                .as_ref()
                .iter()
                .fold((vec![], vec![]), |(mut chars, mut build), b| {
                    build.push(*b);
                    if build.len() == 4 {
                        let u = u32::from_le_bytes(build[0..4].try_into().unwrap());
                        dbg!(&u);
                        chars.push(std::char::from_u32(u).unwrap());
                        build = vec![];
                    }
                    (chars, build)
                });
        assert_eq!(build.len(), 0);
        Ok(chars.iter().collect::<String>())
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
/// - foo/bar
/// - foo/bar/
/// - /foo/bar
/// - /foo/bar/
/// - foo//bar
///
/// there is no normalisation of paths, e.g. to guarantee a specific root component exists, at this
/// layer so there is a risk that there are hash collisions with other data on the DHT network if
/// some disambiguation logic is not included in higher level abstractions.
///
/// this supports sharding strategies from a small inline DSL
/// start each component with <width>:<depth># to get shards out of the string
///
/// e.g.
/// - foo/barbaz => normal path as above ["foo", "barbaz"]
/// - foo/1:3#barbazii => width 1, depth 3, ["foo", "b", "a", "r", "barbazii"]
/// - foo/2:3#barbazii => width 2, depth 3, ["foo", "ba", "rb", "az", "barbazii"]
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

impl From<&Path> for EntryDefId {
    fn from(_: &Path) -> Self {
        Path::entry_def_id()
    }
}

impl Path {
    pub fn entry_def_id() -> EntryDefId {
        core::str::from_utf8(&NAME).unwrap().into()
    }

    pub fn crdt_type() -> CrdtType {
        CrdtType
    }

    pub fn required_validations() -> RequiredValidations {
        RequiredValidations::default()
    }

    pub fn entry_visibility() -> EntryVisibility {
        EntryVisibility::Public
    }

    pub fn entry_def() -> EntryDef {
        EntryDef {
            id: Self::entry_def_id(),
            crdt_type: Self::crdt_type(),
            required_validations: Self::required_validations(),
            visibility: Self::entry_visibility(),
        }
    }

    /// what is the hash for the current Path
    /// something like `$PWD` for the Path, but from a DHT perspective (i.e. a hash)
    pub fn pwd(&self) -> Result<holo_hash_core::HoloHashCore, WasmError> {
        Ok(entry_hash!(self)?)
    }

    /// does an entry exist at the hash we expect?
    /// something like `[ -d $DIR ]`
    pub fn exists(&self) -> Result<bool, WasmError> {
        debug!(get_entry!(self.pwd()?))?;
        Ok(get_entry!(self.pwd()?)?.is_some())
    }

    /// recursively touch this and every parent that doesn't exist yet
    /// something like `mkdir -p $DIR`
    pub fn touch(&self) -> Result<(), WasmError> {
        if !self.exists()? {
            commit_entry!(self)?;
            let parent_vec: Vec<Component> = self.as_ref()[0..self.as_ref().len() - 1].to_vec();
            if parent_vec.len() > 0 {
                let parent = Self::from(parent_vec);
                parent.touch()?;
                link_entries!(
                    parent.pwd()?,
                    self.pwd()?,
                    holochain_zome_types::link::LinkTag::new(NAME)
                )?;
            }
        }
        Ok(())
    }

    /// touch and list all the links from this anchor to anchors below it
    /// only returns links between anchors, not to other entries that might have their own links
    /// something like `mkdir -p $DIR && ls -d $DIR`
    pub fn ls(&self) -> Result<holochain_zome_types::link::Links, WasmError> {
        Self::touch(&self)?;
        Ok(get_links!(
            self.pwd()?,
            holochain_zome_types::link::LinkTag::new(NAME)
        )?)
    }
}

#[test]
#[cfg(test)]
fn hash_path_delimiter() {
    assert_eq!("/", DELIMITER,);
}

#[test]
#[cfg(test)]
fn hash_path_linktag() {
    assert_eq!("hdk.path".as_bytes(), NAME);
}

#[test]
#[cfg(test)]
fn hash_path_component() {
    use fixt::prelude::*;

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
}

#[test]
#[cfg(test)]
fn hash_path_path() {
    use fixt::prelude::*;

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
        ("/", vec![]),
        ("/foo", vec![Component::from("foo")]),
        ("foo", vec![Component::from("foo")]),
        ("foo/", vec![Component::from("foo")]),
        ("/foo/", vec![Component::from("foo")]),
        (
            "/foo/bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "/foo/bar/",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo/bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo/bar/",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo//bar",
            vec![Component::from("foo"), Component::from("bar")],
        ),
        (
            "foo/1:3#abcdef",
            vec![
                Component::from("foo"),
                Component::from("a"),
                Component::from("b"),
                Component::from("c"),
                Component::from("abcdef"),
            ],
        ),
        (
            "foo/2:3#zzzzzzzzzz",
            vec![
                Component::from("foo"),
                Component::from("zz"),
                Component::from("zz"),
                Component::from("zz"),
                Component::from("zzzzzzzzzz"),
            ],
        ),
        (
            "foo/1:3#abcdef/bar",
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
