//! The HashString type is defined here. It is used for type safety throughout the codebase
//! to keep track of places where a string is the product of a hash function,
//! and as a base type for Address to use.

// TODO: remove this module and replace with HoloHash

use multihash::{encode, Hash};
use rust_base58::{FromBase58, ToBase58};
use std::{convert::TryInto, fmt};

/// the default hashing algorithm
pub const DEFAULT_HASH: Hash = Hash::SHA2256;

/// HashString newtype for String
#[derive(
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Clone,
    Debug,
    crate::Serialize,
    crate::Deserialize,
    Default,
    Hash,
)]
pub struct HashString(String);

impl fmt::Display for HashString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for HashString {
    fn from(s: String) -> HashString {
        HashString(s)
    }
}

impl From<HashString> for String {
    fn from(h: HashString) -> String {
        h.0
    }
}

impl<'a> From<&'a str> for HashString {
    fn from(s: &str) -> HashString {
        HashString::from(s.to_string())
    }
}

impl<'a> From<&Vec<u8>> for HashString {
    fn from(v: &Vec<u8>) -> HashString {
        HashString::from(v.clone())
    }
}

impl From<Vec<u8>> for HashString {
    fn from(v: Vec<u8>) -> HashString {
        HashString::from(v.to_base58())
    }
}

impl TryInto<Vec<u8>> for HashString {
    type Error = rust_base58::base58::FromBase58Error;
    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        self.0.from_base58()
    }
}

impl<'a> TryInto<Vec<u8>> for &'a HashString {
    type Error = rust_base58::base58::FromBase58Error;
    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        self.clone().try_into()
    }
}

impl<'a> AsRef<[u8]> for HashString {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl HashString {
    /// construct a new HashString
    pub fn new() -> HashString {
        HashString("".to_string())
    }

    /// convert bytes to a b58 hashed string
    pub fn encode_from_bytes(bytes: &[u8], hash_type: Hash) -> HashString {
        HashString::from(encode(hash_type, bytes).unwrap().to_base58())
    }
}

// @TODO reinstate these in a better way (not introducting deps on types)
// #[cfg(test)]
// pub mod tests {
//     use super::*;
//     use crate::persistence::{
//         cas::content::Addressable,
//         fixture::{test_entry_a, test_hash_a},
//     };
//     use multihash::Hash;
//
//     #[test]
//     /// show From<String> implementation
//     fn from_string_test() {
//         assert_eq!(HashString::new(), HashString::from("".to_string()),);
//
//         assert_eq!(
//             test_hash_a(),
//             HashString::from(test_entry_a().address().to_string()),
//         );
//     }
//
//     #[test]
//     /// show From<&str> implementation
//     fn from_str_test() {
//         assert_eq!(HashString::new(), HashString::from(""));
//
//         assert_eq!(test_hash_a(), test_entry_a().address(),);
//     }
//
//     #[test]
//     /// mimics tests from legacy golang holochain core hashing bytes
//     fn bytes_to_b58_known_golang() {
//         assert_eq!(
//             HashString::encode_from_bytes(b"test data", Hash::SHA2256).to_string(),
//             "QmY8Mzg9F69e5P9AoQPYat655HEhc1TVGs11tmfNSzkqh2"
//         )
//     }
//
//     #[test]
//     fn can_convert_vec_u8_to_hash() {
//         let v: Vec<u8> = vec![48, 49, 50];
//         let hash_string: HashString = v.clone().into();
//         assert_eq!("HBrq", hash_string.to_string());
//         let hash_string_from_ref: HashString = (&v).into();
//         assert_eq!("HBrq", hash_string_from_ref.to_string());
//         let result: Result<Vec<u8>, _> = hash_string.clone().try_into();
//         assert!(result.is_ok());
//         assert_eq!(result.unwrap(), [48, 49, 50]);
//         let result_from_ref: Result<Vec<u8>, _> = (&hash_string).try_into();
//         assert!(result_from_ref.is_ok());
//         assert_eq!(result_from_ref.unwrap(), [48, 49, 50]);
//     }
// }
