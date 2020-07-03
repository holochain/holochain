use holochain_wasmer_guest::*;

/// allows for "foo/bar/baz" to automatically move to/from ["foo", "bar", "baz"] components
/// technically it's moving each string component in as bytes
/// if this is a problem for you simply built the components yourself
/// @see `impl From<String> for Path` below
pub const DELIMITER: &str = "/";

#[derive(
    Clone, PartialEq, Debug, Default, serde::Deserialize, serde::Serialize, SerializedBytes,
)]
#[repr(transparent)]
pub struct Component(#[serde(with = "serde_bytes")] Vec<u8>);

impl From<Vec<u8>> for Component {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl AsRef<[u8]> for Component {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<Component> for Vec<u8> {
    fn from(component: Component) -> Self {
        component.0
    }
}

impl From<&str> for Component {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

/// building a string from a Component can fail because a Component can contain arbitrary bytes
impl std::convert::TryFrom<Component> for String {
    #[derive(Clone, PartialEq, Default, serde::Deserialize, serde::Serialize, SerializedBytes)]
#[repr(transparent)]
pub struct Path(Vec<Component>);

impl From<Vec<Component>> for Path {
    fn from(components: Vec<Component>) -> Self {
        Self(components)
    }
}

impl AsRef<Vec<Component>> for Path {
    fn as_ref(&self) -> &Vec<Component> {
        self.0.as_ref()
    }
}

impl From<String> for Path {
    fn from(s: String) -> Self {
        Self(
            s.split(DELIMITER)
                .map(|s| Component::from(s.as_bytes().to_vec()))
                .collect(),
        )
    }
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
        ("", Path::from(vec![])),
        ("/", Path::from(vec![])),
        ("/foo", Path::from(vec![Component::from("foo")
        }
        }
    }
}
