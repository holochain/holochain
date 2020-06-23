use crate::crdt::CrdtType;
use crate::zome::ZomeName;
use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Copy)]
pub enum EntryVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequiredValidations(u8);

impl From<u8> for RequiredValidations {
    fn from(u: u8) -> Self {
        Self(u)
    }
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
    pub required_validations: RequiredValidations,
}

impl EntryDef {
    pub fn new(
        id: EntryDefId,
        visibility: EntryVisibility,
        crdt_type: CrdtType,
        required_validations: RequiredValidations,
    ) -> Self {
        Self {
            id,
            visibility,
            crdt_type,
            required_validations,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDefs(Vec<EntryDef>);

impl EntryDefs {
    pub fn entry_def_id_position(&self, entry_def_id: EntryDefId) -> Option<usize> {
        self.0
            .iter()
            .position(|entry_def| entry_def.id == entry_def_id)
    }
}

impl std::ops::Index<usize> for EntryDefs {
    type Output = EntryDef;
    fn index(&self, i: usize) -> &Self::Output {
        &self.0[i]
    }
}

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

    use super::EntryDef;
    use super::EntryDefsCallbackResult;
    use super::EntryVisibility;
    use crate::crdt::CrdtType;
    use crate::zome_io::GuestOutput;
    use std::convert::TryInto;

    #[test]
    fn from_guest_output_test() {
        let defs_callback_result = EntryDefsCallbackResult::Defs(
            "foo".into(),
            vec![EntryDef {
                id: "bar".into(),
                visibility: EntryVisibility::Public,
                crdt_type: CrdtType,
                required_validations: 5.into(),
            }]
            .into(),
        );
        let guest_output = GuestOutput::new(defs_callback_result.clone().try_into().unwrap());
        assert_eq!(defs_callback_result, guest_output.into(),);
    }
}
