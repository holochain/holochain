use crate::{entry::Entry, error::HolochainError};

use holochain_persistence_api::cas::content::{Address, AddressableContent};

use holochain_json_api::{error::JsonError, json::JsonString};

use std::{collections::BTreeMap, str::FromStr};

//--------------------------------------------------------------------------------------------------
// CapabilityType
//--------------------------------------------------------------------------------------------------

/// Enum for CapabilityType.  Public capabilities require public grant token.  Transferable
/// capabilities require a token, but don't limit the capability to specific agent(s);
/// this functions like a password in that you can give the token to someone else and it works.
/// Assigned capabilities check the request's signature against the list of agents to which
/// the capability has been granted.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub enum CapabilityType {
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "transferable")]
    Transferable,
    #[serde(rename = "assigned")]
    Assigned,
}

impl Default for CapabilityType {
    fn default() -> CapabilityType {
        CapabilityType::Assigned
    }
}

#[derive(Debug, PartialEq)]
/// Enumeration of all Capabilities known and used by HC Core
/// Enumeration converts to str
pub enum ReservedCapabilityId {
    /// used for identifying the default public capability
    Public,
}

impl FromStr for ReservedCapabilityId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hc_public" => Ok(ReservedCapabilityId::Public),
            _ => Err("Cannot convert string to ReservedCapabilityId"),
        }
    }
}

impl ReservedCapabilityId {
    pub fn as_str(&self) -> &'static str {
        match *self {
            ReservedCapabilityId::Public => "hc_public",
        }
    }
}

pub type CapTokenValue = Address;

/// a collection functions by zome name that are authorized within a capability
pub type CapFunctions = BTreeMap<String, Vec<String>>;

/// System entry to hold a capability token claim for use as a caller
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DefaultJson, Eq)]
pub struct CapTokenClaim {
    id: String,
    grantor: Address,
    token: CapTokenValue,
}

impl CapTokenClaim {
    pub fn new(id: String, grantor: Address, token: CapTokenValue) -> Self {
        CapTokenClaim { id, grantor, token }
    }
    pub fn token(&self) -> CapTokenValue {
        self.token.clone()
    }
    pub fn id(&self) -> String {
        self.id.clone()
    }
    pub fn grantor(&self) -> Address {
        self.grantor.clone()
    }
}

/// System entry to hold a capabilities granted by the callee
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DefaultJson, Eq)]
pub struct CapTokenGrant {
    id: String,
    assignees: Option<Vec<Address>>,
    functions: CapFunctions,
}

impl CapTokenGrant {
    fn new(id: &str, assignees: Option<Vec<Address>>, functions: CapFunctions) -> Self {
        CapTokenGrant {
            id: String::from(id),
            assignees,
            functions,
        }
    }

    pub fn create(
        id: &str,
        cap_type: CapabilityType,
        assignees: Option<Vec<Address>>,
        functions: CapFunctions,
    ) -> Result<Self, HolochainError> {
        let assignees = CapTokenGrant::valid(cap_type, assignees)?;
        Ok(CapTokenGrant::new(id, assignees, functions))
    }

    // internal check that type and assignees are valid for create
    fn valid(
        cap_type: CapabilityType,
        assignees: Option<Vec<Address>>,
    ) -> Result<Option<Vec<Address>>, HolochainError> {
        if (cap_type == CapabilityType::Public || cap_type == CapabilityType::Transferable)
            && (assignees.is_some() && !assignees.clone().unwrap().is_empty())
        {
            return Err(HolochainError::new(
                "there must be no assignees for public or transferable grants",
            ));
        }
        match cap_type {
            CapabilityType::Assigned => {
                if assignees.is_none() || assignees.clone().unwrap().is_empty() {
                    return Err(HolochainError::new(
                        "Assigned grant must have 1 or more assignees",
                    ));
                }
                Ok(assignees)
            }
            CapabilityType::Public => Ok(None),
            CapabilityType::Transferable => Ok(Some(Vec::new())),
        }
    }

    pub fn id(&self) -> String {
        self.id.to_string()
    }

    // the token value is address of the entry, so we can just build it
    // and take the address.
    pub fn token(&self) -> CapTokenValue {
        let addr: Address = Entry::CapTokenGrant((*self).clone()).address();
        addr
    }

    pub fn cap_type(&self) -> CapabilityType {
        match self.assignees() {
            None => CapabilityType::Public,
            Some(vec) => {
                if vec.is_empty() {
                    CapabilityType::Transferable
                } else {
                    CapabilityType::Assigned
                }
            }
        }
    }

    pub fn assignees(&self) -> Option<Vec<Address>> {
        self.assignees.clone()
    }

    pub fn functions(&self) -> CapFunctions {
        self.functions.clone()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    /// test that ReservedCapabilityId can be created from a canonical string
    fn test_reserved_capid_from_str() {
        assert_eq!(
            Ok(ReservedCapabilityId::Public),
            ReservedCapabilityId::from_str("hc_public"),
        );
        assert_eq!(
            Err("Cannot convert string to ReservedCapabilityId"),
            ReservedCapabilityId::from_str("foo"),
        );
    }

    #[test]
    /// test that a canonical string can be created from ReservedCapabilityId
    fn test_reserved_capid_as_str() {
        assert_eq!(ReservedCapabilityId::Public.as_str(), "hc_public");
    }

    #[test]
    fn test_new_cap_token_claim_entry() {
        let token = Address::from("fake");
        let grantor = Address::from("fake grantor");
        let claim = CapTokenClaim::new("foo".to_string(), grantor.clone(), token.clone());
        assert_eq!(claim.id(), "foo".to_string());
        assert_eq!(claim.grantor(), grantor);
        assert_eq!(claim.token(), token);
    }

    #[test]
    fn test_new_cap_token_grant_entry() {
        let empty_functions = CapFunctions::new();
        let grant = CapTokenGrant::new("foo", None, empty_functions.clone());
        assert_eq!(grant.cap_type(), CapabilityType::Public);
        assert_eq!(grant.id(), "foo".to_string());
        let grant = CapTokenGrant::new("", Some(Vec::new()), empty_functions.clone());
        assert_eq!(grant.cap_type(), CapabilityType::Transferable);
        let test_address = Address::new();
        let grant = CapTokenGrant::new(
            "",
            Some(vec![test_address.clone()]),
            empty_functions.clone(),
        );
        assert_eq!(grant.cap_type(), CapabilityType::Assigned);
        assert_eq!(grant.assignees().unwrap()[0], test_address)
    }

    #[test]
    fn test_cap_grant_valid() {
        assert!(CapTokenGrant::valid(CapabilityType::Public, None).is_ok());
        assert!(CapTokenGrant::valid(CapabilityType::Public, Some(Vec::new())).is_ok());
        assert!(CapTokenGrant::valid(CapabilityType::Public, Some(vec![Address::new()])).is_err());
        assert!(CapTokenGrant::valid(CapabilityType::Transferable, None).is_ok());
        assert!(CapTokenGrant::valid(CapabilityType::Transferable, Some(Vec::new())).is_ok());
        assert!(
            CapTokenGrant::valid(CapabilityType::Transferable, Some(vec![Address::new()])).is_err()
        );
        assert!(CapTokenGrant::valid(CapabilityType::Assigned, None).is_err());
        assert!(CapTokenGrant::valid(CapabilityType::Assigned, Some(Vec::new())).is_err());
        assert!(CapTokenGrant::valid(CapabilityType::Assigned, Some(vec![Address::new()])).is_ok());
    }

    #[test]
    fn test_create_cap_token_grant_entry() {
        let some_fn = String::from("some_fn");
        let mut example_functions = CapFunctions::new();
        example_functions.insert("some_zome".to_string(), vec![some_fn]);
        let maybe_grant = CapTokenGrant::create(
            "foo",
            CapabilityType::Public,
            None,
            example_functions.clone(),
        );
        assert!(maybe_grant.is_ok());
        let grant = maybe_grant.unwrap();
        assert_eq!(grant.id, "foo".to_string());
        assert_eq!(grant.cap_type(), CapabilityType::Public);
        assert_eq!(grant.functions(), example_functions.clone());

        let maybe_grant = CapTokenGrant::create(
            "foo",
            CapabilityType::Transferable,
            Some(Vec::new()),
            example_functions.clone(),
        );
        assert!(maybe_grant.is_ok());
        let grant = maybe_grant.unwrap();
        assert_eq!(grant.cap_type(), CapabilityType::Transferable);

        let test_address = Address::new();

        let maybe_grant = CapTokenGrant::create(
            "foo",
            CapabilityType::Public,
            Some(vec![test_address.clone()]),
            example_functions.clone(),
        );
        assert!(maybe_grant.is_err());
        let maybe_grant = CapTokenGrant::create(
            "foo",
            CapabilityType::Transferable,
            None,
            example_functions.clone(),
        );
        assert!(maybe_grant.is_ok());
        let grant = maybe_grant.unwrap();
        assert_eq!(grant.cap_type(), CapabilityType::Transferable);

        let maybe_grant = CapTokenGrant::create(
            "foo",
            CapabilityType::Assigned,
            Some(vec![test_address.clone()]),
            example_functions.clone(),
        );
        assert!(maybe_grant.is_ok());
        let grant = maybe_grant.unwrap();
        assert_eq!(grant.cap_type(), CapabilityType::Assigned);
        assert_eq!(grant.assignees().unwrap()[0], test_address)
    }
}
