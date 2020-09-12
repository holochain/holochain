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

impl From<u8> for EntryDefIndex {
    fn from(a: u8) -> Self {
        Self(a)
    }
}

#[derive(Debug, Clone)]
pub struct WrongHeaderError(pub String);

impl std::fmt::Display for WrongHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tried to unwrap a Header to the wrong variant")
    }
}

impl std::error::Error for WrongHeaderError {}

impl TryFrom<Header> for UpdateEntry {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::UpdateEntry(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a UpdateEntry {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::UpdateEntry(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for DeleteElement {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::DeleteElement(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a DeleteElement {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::DeleteElement(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for LinkAdd {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::LinkAdd(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a LinkAdd {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::LinkAdd(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for LinkRemove {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::LinkRemove(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a LinkRemove {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::LinkRemove(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}
