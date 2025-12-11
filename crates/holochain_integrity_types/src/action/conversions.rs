use super::*;
use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

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

#[derive(PartialEq, Eq, Debug, Clone, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub struct WrongActionError(pub String);

impl std::fmt::Display for WrongActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tried to unwrap an action to the wrong variant")
    }
}

impl std::error::Error for WrongActionError {}

impl TryFrom<Action> for Update {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::Update(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for &'a Update {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::Update(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl TryFrom<Action> for Delete {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::Delete(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for &'a Delete {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::Delete(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl TryFrom<Action> for CreateLink {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::CreateLink(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for &'a CreateLink {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::CreateLink(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl TryFrom<Action> for DeleteLink {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::DeleteLink(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for &'a DeleteLink {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::DeleteLink(h) => Ok(h),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}
