use super::*;

impl From<u8> for ZomeIndex {
    fn from(a: u8) -> Self {
        Self(a)
    }
}

impl From<ZomeIndex> for u8 {
    fn from(a: ZomeIndex) -> Self {
        a.0
    }
}

impl std::fmt::Display for ZomeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u8> for EntryDefIndex {
    fn from(a: u8) -> Self {
        Self(a)
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct WrongActionError(pub String);

impl std::fmt::Display for WrongActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tried to unwrap an action to the wrong variant")
    }
}

impl std::error::Error for WrongActionError {}
