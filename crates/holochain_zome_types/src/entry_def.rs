use crate::crdt::CrdtType;
use crate::validate::RequiredValidationType;
use crate::zome_io::ExternOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

const DEFAULT_REQUIRED_VALIDATIONS: u8 = 5;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EntryDefId {
    App(String),
    CapClaim,
    CapGrant,
}

impl From<String> for EntryDefId {
    fn from(s: String) -> Self {
        Self::App(s)
    }
}

impl From<&str> for EntryDefId {
    fn from(s: &str) -> Self {
        Self::App(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Copy, Hash)]
pub enum EntryVisibility {
    Public,
    Private,
}

impl Default for EntryVisibility {
    fn default() -> Self {
        Self::Public
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequiredValidations(u8);

impl From<u8> for RequiredValidations {
    fn from(u: u8) -> Self {
        Self(u)
    }
}

impl From<RequiredValidations> for u8 {
    fn from(required_validations: RequiredValidations) -> Self {
        required_validations.0
    }
}

impl Default for RequiredValidations {
    fn default() -> Self {
        Self(DEFAULT_REQUIRED_VALIDATIONS)
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
    /// The required validation package for this entry
    pub required_validation_type: RequiredValidationType,
}

impl EntryDef {
    pub fn new(
        id: EntryDefId,
        visibility: EntryVisibility,
        crdt_type: CrdtType,
        required_validations: RequiredValidations,
        required_validation_type: RequiredValidationType,
    ) -> Self {
        Self {
            id,
            visibility,
            crdt_type,
            required_validations,
            required_validation_type,
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn default_with_id<I: Into<EntryDefId>>(id: I) -> Self {
        EntryDef::new(
            id.into(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
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

impl IntoIterator for EntryDefs {
    type Item = EntryDef;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Vec<EntryDef>> for EntryDefs {
    fn from(v: Vec<EntryDef>) -> Self {
        Self(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum EntryDefsCallbackResult {
    Defs(EntryDefs),
    Err(String),
}

impl From<Vec<EntryDef>> for EntryDefsCallbackResult {
    fn from(v: Vec<EntryDef>) -> Self {
        Self::Defs(v.into())
    }
}

impl From<ExternOutput> for EntryDefsCallbackResult {
    fn from(callback_guest_output: ExternOutput) -> Self {
        match callback_guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Err(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for EntryDefsCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            EntryDefsCallbackResult::Defs(_) => false,
            EntryDefsCallbackResult::Err(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EntryDef;
    use super::EntryDefsCallbackResult;
    use super::EntryVisibility;
    use crate::crdt::CrdtType;
    use crate::validate::RequiredValidationType;
    use crate::zome_io::ExternOutput;
    use std::convert::TryInto;

    #[test]
    fn from_guest_output_test() {
        let defs_callback_result = EntryDefsCallbackResult::Defs(
            vec![EntryDef {
                id: "bar".into(),
                visibility: EntryVisibility::Public,
                crdt_type: CrdtType,
                required_validations: 5.into(),
                required_validation_type: RequiredValidationType::default(),
            }]
            .into(),
        );
        let guest_output = ExternOutput::new(defs_callback_result.clone().try_into().unwrap());
        assert_eq!(defs_callback_result, guest_output.into(),);
    }
}
