//! Implements a definition of what AddressableContent is by defining Content,
//! defining Address, and defining the relationship between them. AddressableContent is a trait,
//! meaning that it can be implemented for other structs.
//! A test suite for AddressableContent is also implemented here.

use crate::persistence::hash::HashString;
use holochain_json_api::{error::JsonError, json::*};

use multihash::Hash;
use std::fmt::Debug;

/// an Address for some Content
/// ideally would be the Content but pragmatically must be Address
/// consider what would happen if we had multi GB addresses...
pub type Address = HashString;

/// the Content is a JsonString
/// this is the only way to be confident in persisting all Rust types across all backends
pub type Content = JsonString;

/// can be stored as serialized content
/// the content is the address, there is no "location" like a file system or URL
/// @see https://en.wikipedia.org/wiki/Content-addressable_storage
pub trait AddressableContent {
    /// the Address the Content would be available at once stored in a ContentAddressableStorage
    /// default implementation is provided as hashing Content with sha256
    /// the default implementation should cover most use-cases
    /// it is critical that there are no hash collisions across all stored AddressableContent
    /// it is recommended to implement an "address space" prefix for address algorithms that don't
    /// offer strong cryptographic guarantees like sha et. al.
    fn address(&self) -> Address {
        Address::encode_from_str(&String::from(self.content()), Hash::SHA2256)
    }

    /// the Content that would be stored in a ContentAddressableStorage
    /// the default implementation covers anything that implements From<Foo> for JsonString
    fn content(&self) -> Content;

    /// restore/deserialize the original struct/type from serialized Content
    /// the default implementation covers anything that implements From<JsonString> for Foo
    fn try_from_content(content: &Content) -> Result<Self, JsonError>
    where
        Self: Sized;
}

impl AddressableContent for Content {
    fn content(&self) -> Content {
        self.clone()
    }

    fn try_from_content(content: &Content) -> Result<Self, JsonError> {
        Ok(content.clone())
    }
}

#[derive(Debug, PartialEq, Clone, Hash, Eq, Deserialize)]
/// some struct that can be content addressed
/// imagine an Entry, ChainHeader, Meta Value, etc.
pub struct ExampleAddressableContent {
    content: Content,
}

impl AddressableContent for ExampleAddressableContent {
    fn content(&self) -> Content {
        self.content.clone()
    }

    fn try_from_content(content: &Content) -> Result<Self, JsonError> {
        Ok(ExampleAddressableContent {
            content: content.clone(),
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
/// another struct that can be content addressed
/// used to show ExampleCas storing multiple types
pub struct OtherExampleAddressableContent {
    content: Content,
    address: Address,
}

/// address is calculated eagerly rather than on call
impl AddressableContent for OtherExampleAddressableContent {
    fn address(&self) -> Address {
        self.address.clone()
    }

    fn content(&self) -> Content {
        self.content.clone()
    }

    fn try_from_content(content: &Content) -> Result<Self, JsonError> {
        Ok(OtherExampleAddressableContent {
            content: content.clone(),
            address: Address::encode_from_str(&String::from(content), Hash::SHA2256),
        })
    }
}

pub struct AddressableContentTestSuite;

impl AddressableContentTestSuite {
    /// test that trait gives the write content
    pub fn addressable_content_trait_test<T>(
        content: Content,
        expected_content: T,
        address: Address,
    ) where
        T: AddressableContent + Debug + PartialEq + Clone,
    {
        let addressable_content = T::try_from_content(&content)
            .expect("could not create AddressableContent from Content");

        assert_eq!(addressable_content, expected_content);
        assert_eq!(content, addressable_content.content());
        assert_eq!(address, addressable_content.address());
    }

    /// test that two different addressable contents would give them same thing
    pub fn addressable_contents_are_the_same_test<T, K>(content: Content)
    where
        T: AddressableContent + Debug + PartialEq + Clone,
        K: AddressableContent + Debug + PartialEq + Clone,
    {
        let addressable_content = T::try_from_content(&content)
            .expect("could not create AddressableContent from Content");
        let other_addressable_content = K::try_from_content(&content)
            .expect("could not create AddressableContent from Content");

        assert_eq!(
            addressable_content.content(),
            other_addressable_content.content()
        );
        assert_eq!(
            addressable_content.address(),
            other_addressable_content.address()
        );
    }
}

#[cfg(test)]
pub mod tests {
    use crate::persistence::cas::content::{
        Address, AddressableContent, AddressableContentTestSuite, ExampleAddressableContent,
        OtherExampleAddressableContent,
    };
    use holochain_json_api::json::{JsonString, RawString};

    #[test]
    /// test the first example
    fn example_addressable_content_trait_test() {
        AddressableContentTestSuite::addressable_content_trait_test::<ExampleAddressableContent>(
            JsonString::from(RawString::from("foo")),
            ExampleAddressableContent::try_from_content(&JsonString::from(RawString::from("foo")))
                .unwrap(),
            Address::from("QmaKze4knhzQPuofhaXfg8kPG3V92MLgDX95xe8g5eafLn"),
        );
    }

    #[test]
    /// test the other example
    fn other_example_addressable_content_trait_test() {
        AddressableContentTestSuite::addressable_content_trait_test::<OtherExampleAddressableContent>(
            JsonString::from(RawString::from("foo")),
            OtherExampleAddressableContent::try_from_content(&JsonString::from(RawString::from(
                "foo",
            )))
            .unwrap(),
            Address::from("QmaKze4knhzQPuofhaXfg8kPG3V92MLgDX95xe8g5eafLn"),
        );
    }

    #[test]
    /// test that both implementations do the same thing
    fn example_addressable_contents_are_the_same_test() {
        AddressableContentTestSuite::addressable_contents_are_the_same_test::<
            ExampleAddressableContent,
            OtherExampleAddressableContent,
        >(JsonString::from(RawString::from("foo")));
    }
}
