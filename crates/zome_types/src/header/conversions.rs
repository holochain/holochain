use super::*;

impl From<u8> for ZomeId {
    fn from(a: u8) -> Self {
        Self(a)
    }
}

impl From<ZomeId> for u8 {
    fn from(a: ZomeId) -> Self {
        a.0
    }
}

impl std::fmt::Display for ZomeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u8> for EntryDefId {
    fn from(a: u8) -> Self {
        Self(a)
    }
}
