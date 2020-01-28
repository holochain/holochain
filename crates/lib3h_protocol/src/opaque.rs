/// Represents an opaque vector of bytes. Lib3h will
/// store or transfer this data but will never inspect
/// or interpret its contents
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize, Hash)]
pub struct Opaque(#[serde(with = "base64")] Vec<u8>);

impl Opaque {
    pub fn new() -> Self {
        Vec::new().into()
    }
    pub fn as_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl From<Opaque> for Vec<u8> {
    fn from(o: Opaque) -> Self {
        o.0
    }
}

impl From<Vec<u8>> for Opaque {
    fn from(vec: Vec<u8>) -> Self {
        Opaque(vec)
    }
}

impl From<&[u8]> for Opaque {
    fn from(bytes: &[u8]) -> Self {
        Opaque(Vec::from(bytes))
    }
}

impl From<String> for Opaque {
    fn from(str: String) -> Self {
        str.as_bytes().into()
    }
}

impl From<&String> for Opaque {
    fn from(str: &String) -> Self {
        str.clone().into()
    }
}

impl From<&str> for Opaque {
    fn from(str: &str) -> Self {
        str.as_bytes().into()
    }
}

impl std::ops::Deref for Opaque {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Opaque {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::fmt::Debug for Opaque {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = String::from_utf8_lossy(self.0.as_ref());
        write!(f, "{:?}", bytes)
    }
}

impl std::fmt::Display for Opaque {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ---------- serialization helper for binary data as base 64 ---------- //

mod base64 {
    extern crate base64;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&base64::display::Base64Display::with_config(
            bytes,
            base64::STANDARD,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String>::deserialize(deserializer)?;
        base64::decode(&s).map_err(de::Error::custom)
    }
}
