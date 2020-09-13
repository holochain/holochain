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

impl TryFrom<Header> for Update {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Update(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a Update {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::Update(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for Delete {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Delete(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a Delete {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::Delete(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for CreateLink {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::CreateLink(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a CreateLink {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::CreateLink(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl TryFrom<Header> for DeleteLink {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::DeleteLink(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a DeleteLink {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::DeleteLink(h) => Ok(h),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}
