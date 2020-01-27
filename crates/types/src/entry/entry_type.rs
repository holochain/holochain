use error::HolochainError;
use holochain_json_api::{error::JsonError, json::JsonString};
use std::{
    convert::TryFrom,
    fmt::{Display, Formatter, Result as FmtResult},
    str::FromStr,
};

// Macro for statically concatanating the system entry prefix for entry types of system entries
macro_rules! sys_prefix {
    ($s:expr) => {
        concat!("%", $s)
    };
}

#[derive(
    Debug, Clone, PartialEq, Hash, Serialize, Deserialize, PartialOrd, Ord, Eq, DefaultJson,
)]
pub struct AppEntryType(String);

impl From<&'static str> for AppEntryType {
    fn from(s: &str) -> Self {
        AppEntryType(s.to_string())
    }
}

impl From<String> for AppEntryType {
    fn from(s: String) -> Self {
        AppEntryType(s)
    }
}

impl From<AppEntryType> for String {
    fn from(app_entry_type: AppEntryType) -> Self {
        app_entry_type.0
    }
}

impl ToString for AppEntryType {
    fn to_string(&self) -> String {
        String::from(self.to_owned())
    }
}

// Enum for listing all System Entry Types
// Variant `Data` is for user defined entry types
#[derive(
    Debug, Clone, PartialEq, Hash, Serialize, Deserialize, DefaultJson, PartialOrd, Ord, Eq,
)]
pub enum EntryType {
    App(AppEntryType),

    Dna,
    AgentId,
    Deletion,
    LinkAdd,
    LinkRemove,
    LinkList,
    ChainHeader,
    ChainMigrate,
    CapTokenGrant,
    CapTokenClaim,
}

impl From<AppEntryType> for EntryType {
    fn from(app_entry_type: AppEntryType) -> Self {
        EntryType::App(app_entry_type)
    }
}

impl TryFrom<EntryType> for AppEntryType {
    type Error = HolochainError;
    fn try_from(entry_type: EntryType) -> Result<Self, Self::Error> {
        match entry_type {
            EntryType::App(app_entry_type) => Ok(app_entry_type),
            _ => Err(HolochainError::ErrorGeneric(format!(
                "Attempted to convert {:?} EntryType to an AppEntryType",
                entry_type
            ))),
        }
    }
}

impl EntryType {
    pub fn is_app(&self) -> bool {
        match self {
            EntryType::App(_) => true,
            _ => false,
        }
    }
    pub fn is_sys(&self) -> bool {
        !self.is_app()
    }
    /// Checks entry_type_name is valid
    pub fn has_valid_app_name(entry_type_name: &str) -> bool {
        // TODO #445 - do a real regex test instead
        // - must not be empty
        // - must not contain any glob wildcards
        !entry_type_name.is_empty()
        // Must not have sys_prefix
            && &entry_type_name[0..1] != "%"
    }
}

impl FromStr for EntryType {
    type Err = usize;
    // Note: Function always return Ok()
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            sys_prefix!("agent_id") => EntryType::AgentId,
            sys_prefix!("deletion") => EntryType::Deletion,
            sys_prefix!("dna") => EntryType::Dna,
            sys_prefix!("chain_header") => EntryType::ChainHeader,
            sys_prefix!("link_add") => EntryType::LinkAdd,
            sys_prefix!("link_remove") => EntryType::LinkRemove,
            sys_prefix!("link_list") => EntryType::LinkList,
            sys_prefix!("chain_migrate") => EntryType::ChainMigrate,
            sys_prefix!("cap_token_claim") => EntryType::CapTokenClaim,
            sys_prefix!("cap_token_grant") => EntryType::CapTokenGrant,
            _ => EntryType::App(AppEntryType(s.into())),
        })
    }
}

impl From<EntryType> for String {
    fn from(entry_type: EntryType) -> String {
        String::from(match entry_type {
            EntryType::App(ref app_entry_type) => &app_entry_type.0,
            EntryType::AgentId => sys_prefix!("agent_id"),
            EntryType::Deletion => sys_prefix!("deletion"),
            EntryType::Dna => sys_prefix!("dna"),
            EntryType::ChainHeader => sys_prefix!("chain_header"),
            EntryType::LinkAdd => sys_prefix!("link_add"),
            EntryType::LinkRemove => sys_prefix!("link_remove"),
            EntryType::LinkList => sys_prefix!("link_list"),
            EntryType::ChainMigrate => sys_prefix!("chain_migrate"),
            EntryType::CapTokenClaim => sys_prefix!("cap_token_claim"),
            EntryType::CapTokenGrant => sys_prefix!("cap_token_grant"),
        })
    }
}

impl From<String> for EntryType {
    fn from(s: String) -> EntryType {
        EntryType::from_str(&s).expect("could not convert String to EntryType")
    }
}

impl From<&'static str> for EntryType {
    fn from(s: &str) -> EntryType {
        EntryType::from(String::from(s))
    }
}

impl Display for EntryType {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", String::from(self.to_owned()))
    }
}

/// dummy entry type
#[cfg_attr(tarpaulin, skip)]
pub fn test_app_entry_type() -> AppEntryType {
    AppEntryType::from("testEntryType")
}

pub fn test_entry_type() -> EntryType {
    EntryType::App(test_app_entry_type())
}

/// dummy entry type, same as test_type()
#[cfg_attr(tarpaulin, skip)]
pub fn test_app_entry_type_a() -> AppEntryType {
    test_app_entry_type()
}

pub fn test_entry_type_a() -> EntryType {
    EntryType::App(test_app_entry_type_a())
}

/// dummy entry type, differs from test_type()
#[cfg_attr(tarpaulin, skip)]
pub fn test_app_entry_type_b() -> AppEntryType {
    AppEntryType::from("testEntryTypeB")
}

pub fn test_entry_type_b() -> EntryType {
    EntryType::App(test_app_entry_type_b())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn test_types() -> Vec<EntryType> {
        vec![
            EntryType::App(AppEntryType::from("foo")),
            EntryType::Dna,
            EntryType::AgentId,
            EntryType::Deletion,
            EntryType::LinkAdd,
            EntryType::LinkRemove,
            EntryType::LinkList,
            EntryType::ChainHeader,
            EntryType::ChainMigrate,
            EntryType::CapTokenClaim,
            EntryType::CapTokenGrant,
        ]
    }

    #[test]
    fn entry_type_kind() {
        assert!(EntryType::App(AppEntryType::from("")).is_app());
        assert!(!EntryType::App(AppEntryType::from("")).is_sys());
        assert!(EntryType::AgentId.is_sys());
        assert!(!EntryType::AgentId.is_app());
    }

    #[test]
    fn entry_type_valid_app_name() {
        assert!(EntryType::has_valid_app_name("agent_id"));
        assert!(!EntryType::has_valid_app_name("%agent_id"));
        assert!(!EntryType::has_valid_app_name(&String::from(
            EntryType::AgentId
        )));
        assert!(!EntryType::has_valid_app_name(&String::new()));
        assert!(EntryType::has_valid_app_name("toto"));
        assert!(!EntryType::has_valid_app_name("%%"));
        // TODO #445 - do a real regex test in has_valid_app_name()
        // assert!(EntryType::has_valid_app_name("\n"));
    }

    #[test]
    fn entry_type_as_str_test() {
        for (type_str, variant) in vec![
            (sys_prefix!("dna"), EntryType::Dna),
            (sys_prefix!("agent_id"), EntryType::AgentId),
            (sys_prefix!("deletion"), EntryType::Deletion),
            (sys_prefix!("link_add"), EntryType::LinkAdd),
            (sys_prefix!("link_remove"), EntryType::LinkRemove),
            (sys_prefix!("link_list"), EntryType::LinkList),
            (sys_prefix!("chain_header"), EntryType::ChainHeader),
            (sys_prefix!("chain_migrate"), EntryType::ChainMigrate),
            (sys_prefix!("cap_token_claim"), EntryType::CapTokenClaim),
            (sys_prefix!("cap_token_grant"), EntryType::CapTokenGrant),
        ] {
            assert_eq!(
                variant,
                EntryType::from_str(type_str).expect("could not convert str to EntryType")
            );

            assert_eq!(type_str, &String::from(variant),);
        }
    }
}
