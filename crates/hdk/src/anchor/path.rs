use holochain_wasmer_guest::*;

/// allows for "foo/bar/baz" to automatically move to/from ["foo", "bar", "baz"] components
/// technically it's moving each string component in as bytes
/// if this is a problem for you simply built the components yourself
/// @see `impl From<String> for Path` below
pub const DELIMITER: &str = "/";

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
impl From<&str> for Component {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

/// building a string from a Component can fail because a Component can contain arbitrary bytes
/// in general it is only safe to create strings from components that were themselves built from a
/// string
/// @see `impl From<&str> for Component`
impl std::convert::TryFrom<Component> for String {
    type Error = std::str::Utf8Error;
    fn try_from(component: Component) -> Result<Self, Self::Error> {
        Ok(std::str::from_utf8(&component.0)?.to_string())
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
impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self(
            s.split(DELIMITER)
                .filter(|s| !s.is_empty())
                .map(|s| Component::from(s.as_bytes().to_vec()) )
                .collect(),
        )
    }
}

#[test]
#[cfg(test)]
fn anchor_delimiter() {
    assert_eq!("/", DELIMITER,);
}

#[test]
#[cfg(test)]
fn anchor_component() {
    use fixt::prelude::*;

    let bytes: Vec<u8> = U8Fixturator::new(fixt::Unpredictable).take(5).collect();

    let component = Component::from(bytes.clone());

    assert_eq!(bytes, component.as_ref(),);
}

#[test]
#[cfg(test)]
fn anchor_path() {
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
        ("/foo/bar", vec![Component::from("foo"), Component::from("bar")]),
        ("/foo/bar/", vec![Component::from("foo"), Component::from("bar")]),
        ("foo/bar", vec![Component::from("foo"), Component::from("bar")]),
        ("foo/bar/", vec![Component::from("foo"), Component::from("bar")]),
        ("foo//bar", vec![Component::from("foo"), Component::from("bar")]),
    ] {
        assert_eq!(Path::from(input), Path::from(output),);
    }
}
