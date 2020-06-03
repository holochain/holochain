use crate::zome::ZomeName;
use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;
use crate::crdt::CrdtType;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDefId(String);

impl From<String> for EntryDefId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EntryDefId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryVisibility {
    Public,
    Private,
}

impl EntryVisibility {
    /// converts entry visibility enum into boolean value on public
    pub fn is_public(&self) -> bool {
        *self == EntryVisibility::Public
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDef {
    /// Zome-unique identifier for this entry type
    pub id: EntryDefId,
    /// Public or Private
    pub visibility: EntryVisibility,
    /// TBD -- Special types of conflict resolution support from Holochain (e.g. Single-Author, )
    pub crdt_type: CrdtType,
    /// how many validations to receive before considered "network saturated" (MAX value of 50?)
    pub required_validations: u8,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDefs(Vec<EntryDef>);

impl From<Vec<EntryDef>> for EntryDefs {
    fn from(v: Vec<EntryDef>) -> Self {
        Self(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum EntryDefsCallbackResult {
    Defs(ZomeName, EntryDefs),
    Err(ZomeName, String),
}

impl From<GuestOutput> for EntryDefsCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Err(ZomeName::unknown(), format!("{:?}", e)),
        }
    }
}

impl CallbackResult for EntryDefsCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            EntryDefsCallbackResult::Defs(_, _) => false,
            EntryDefsCallbackResult::Err(_, _) => true,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::EntryDefsCallbackResult;
    use super::EntryVisibility;
    use crate::zome_io::GuestOutput;
    use super::EntryDef;
    use crate::crdt::CrdtType;
    use std::convert::TryInto;

    #[test]
    fn from_guest_output_test() {
        let defs_callback_result = EntryDefsCallbackResult::Defs("foo".into(), vec![EntryDef {
            id: "bar".into(),
            visibility: EntryVisibility::Public,
            crdt_type: CrdtType,
            required_validations: 5,
        }].into());
        let guest_output = GuestOutput::new(defs_callback_result.clone().try_into().unwrap());
        assert_eq!(
            defs_callback_result,
            guest_output.into(),
        );
    }

}
