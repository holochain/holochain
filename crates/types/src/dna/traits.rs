//! Module for zome trait structures

use std::str::FromStr;

#[derive(Debug, PartialEq)]
/// Enumeration of all Traits known and used by HC Core
/// Enumeration converts to str
pub enum ReservedTraitNames {
    /// Development placeholder, no production fn should use MissingNo
    MissingNo,

    /// used for declaring functions that will auto-generate a public grant during init
    Public,
}

impl FromStr for ReservedTraitNames {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hc_public" => Ok(ReservedTraitNames::Public),
            _ => Err("Cannot convert string to ReservedTraitNames"),
        }
    }
}

impl ReservedTraitNames {
    pub fn as_str(&self) -> &'static str {
        match *self {
            ReservedTraitNames::Public => "hc_public",
            ReservedTraitNames::MissingNo => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// test that ReservedTraitNames can be created from a canonical string
    fn test_traits_from_str() {
        assert_eq!(
            Ok(ReservedTraitNames::Public),
            ReservedTraitNames::from_str("hc_public"),
        );
        assert_eq!(
            Err("Cannot convert string to ReservedTraitNames"),
            ReservedTraitNames::from_str("foo"),
        );
    }

    #[test]
    /// test that a canonical string can be created from ReservedTraitNames
    fn test_reserved_traits_as_str() {
        assert_eq!(ReservedTraitNames::Public.as_str(), "hc_public");
    }
}
