//! This module contains definitions of the ChainHeader struct, constructor, and getters. This struct really defines a local source chain,
//! in the sense that it implements the pointers between hashes that a hash chain relies on, which
//! are then used to check the integrity of data using cryptographic hash functions.

use crate::{
    agent::test_agent_id,
    entry::{
        entry_type::{test_entry_type, EntryType},
        test_entry,
    },
    signature::{Provenance, Signature},
    time::{test_iso_8601, Iso8601},
};

use holochain_persistence_api::cas::content::{Address, AddressableContent, Content};

use holochain_json_api::{
    error::{JsonError, JsonResult},
    json::JsonString,
};

use std::convert::TryInto;

/// ChainHeader of a source chain "Item"
/// The address of the ChainHeader is used as the Item's key in the source chain hash table
/// ChainHeaders are linked to next header in chain and next header of same type in chain
// @TODO - serialize properties as defined in ChainHeadersEntrySchema from golang alpha 1
// @see https://github.com/holochain/holochain-proto/blob/4d1b8c8a926e79dfe8deaa7d759f930b66a5314f/entry_headers.go#L7
// @see https://github.com/holochain/holochain-rust/issues/75
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, Eq)]
pub struct ChainHeader {
    /// the type of this entry
    /// system types may have associated "subconscious" behavior
    entry_type: EntryType,
    /// Key to the entry of this header
    entry_address: Address,
    /// Address(es) of the agent(s) that authored and signed this entry,
    /// along with their cryptographic signatures
    provenances: Vec<Provenance>,
    /// Key to the immediately preceding header. Only the init Pair can have None as valid
    link: Option<Address>,
    /// Key to the most recent header of the same type, None is valid only for the first of that type
    link_same_type: Option<Address>,
    /// Key to the header of the previous version of this chain header's entry
    link_update_delete: Option<Address>,
    /// ISO8601 time stamp
    timestamp: Iso8601,
}

impl PartialEq for ChainHeader {
    fn eq(&self, other: &ChainHeader) -> bool {
        self.address() == other.address()
    }
}

impl ChainHeader {
    /// build a new ChainHeader from a chain, entry type and entry.
    /// a ChainHeader is immutable, but the chain is mutable if chain.push() is used.
    /// this means that a header becomes invalid and useless as soon as the chain is mutated
    /// the only valid usage of a header is to immediately push it onto a chain in a Pair.
    /// normally (outside unit tests) the generation of valid headers is internal to the
    /// chain::SourceChain trait and should not need to be handled manually
    ///
    /// @see chain::entry::Entry
    pub fn new(
        entry_type: &EntryType,
        entry_address: &Address,
        provenances: &[Provenance],
        link: &Option<Address>,
        link_same_type: &Option<Address>,
        link_update_delete: &Option<Address>,
        timestamp: &Iso8601,
    ) -> Self {
        ChainHeader {
            entry_type: entry_type.to_owned(),
            entry_address: entry_address.to_owned(),
            provenances: provenances.to_owned(),
            link: link.to_owned(),
            link_same_type: link_same_type.to_owned(),
            link_update_delete: link_update_delete.to_owned(),
            timestamp: timestamp.to_owned(),
        }
    }

    /// entry_type getter
    pub fn entry_type(&self) -> &EntryType {
        &self.entry_type
    }

    /// timestamp getter
    pub fn timestamp(&self) -> &Iso8601 {
        &self.timestamp
    }

    /// link getter
    pub fn link(&self) -> Option<Address> {
        self.link.clone()
    }

    /// entry_address getter
    pub fn entry_address(&self) -> &Address {
        &self.entry_address
    }

    /// link_same_type getter
    pub fn link_same_type(&self) -> Option<Address> {
        self.link_same_type.clone()
    }

    /// link_update_delete getter
    pub fn link_update_delete(&self) -> Option<Address> {
        self.link_update_delete.clone()
    }

    /// entry_signature getter
    pub fn provenances(&self) -> &Vec<Provenance> {
        &self.provenances
    }
}

impl AddressableContent for ChainHeader {
    fn content(&self) -> Content {
        self.to_owned().into()
    }

    fn try_from_content(content: &Content) -> JsonResult<Self> {
        content.to_owned().try_into()
    }
}

/// returns a dummy header for use in tests
pub fn test_chain_header() -> ChainHeader {
    test_chain_header_with_sig("sig")
}

/// returns a dummy header for use in tests
pub fn test_chain_header_with_sig(sig: &'static str) -> ChainHeader {
    ChainHeader::new(
        &test_entry_type(),
        &test_entry().address(),
        &test_provenances(sig),
        &None,
        &None,
        &None,
        &test_iso_8601(),
    )
}

pub fn test_provenances(sig: &'static str) -> Vec<Provenance> {
    vec![Provenance::new(
        test_agent_id().address(),
        Signature::from(sig),
    )]
}

#[cfg(test)]
pub mod tests {
    use crate::{
        chain_header::{test_chain_header, test_provenances, ChainHeader},
        entry::{
            entry_type::{test_entry_type, test_entry_type_a, test_entry_type_b},
            test_entry, test_entry_a, test_entry_b,
        },
        time::test_iso_8601,
    };
    use holochain_persistence_api::cas::content::{Address, AddressableContent};

    /// returns a dummy header for use in tests
    pub fn test_chain_header_a() -> ChainHeader {
        test_chain_header()
    }

    /// returns a dummy header for use in tests. different from test_chain_header_a.
    pub fn test_chain_header_b() -> ChainHeader {
        ChainHeader::new(
            &test_entry_type_b(),
            &test_entry_b().address(),
            &test_provenances("sig"),
            &None,
            &None,
            &None,
            &test_iso_8601(),
        )
    }

    pub fn test_header_address() -> Address {
        Address::from("Qmc1n5gbUU2QKW6is9ENTqmaTcEjYMBwNkcACCxe3bBDnd".to_string())
    }

    #[test]
    /// tests for PartialEq
    fn eq() {
        // basic equality
        assert_eq!(test_chain_header(), test_chain_header());

        // different content is different
        assert_ne!(test_chain_header_a(), test_chain_header_b());

        // different type is different
        let entry_a = test_entry_a();
        let entry_b = test_entry_b();
        assert_ne!(
            ChainHeader::new(
                &entry_a.entry_type(),
                &entry_a.address(),
                &test_provenances("sig"),
                &None,
                &None,
                &None,
                &test_iso_8601(),
            ),
            ChainHeader::new(
                &entry_b.entry_type(),
                &entry_a.address(),
                &test_provenances("sig"),
                &None,
                &None,
                &None,
                &test_iso_8601(),
            ),
        );

        // different previous header is different
        let entry = test_entry();
        assert_ne!(
            ChainHeader::new(
                &entry.entry_type(),
                &entry.address(),
                &test_provenances("sig"),
                &None,
                &None,
                &None,
                &test_iso_8601(),
            ),
            ChainHeader::new(
                &entry.entry_type(),
                &entry.address(),
                &test_provenances("sig"),
                &Some(test_chain_header().address()),
                &None,
                &None,
                &test_iso_8601(),
            ),
        );
    }

    #[test]
    /// tests for ChainHeader::new()
    fn new() {
        let chain_header = test_chain_header();

        assert_eq!(chain_header.entry_address(), &test_entry().address());
        assert_eq!(chain_header.link(), None);
        assert_ne!(chain_header.address(), Address::new());
    }

    #[test]
    /// tests for header.entry_type()
    fn entry_type() {
        assert_eq!(test_chain_header().entry_type(), &test_entry_type());
    }

    #[test]
    /// tests for header.time()
    fn timestamp_test() {
        assert_eq!(test_chain_header().timestamp(), &test_iso_8601());
    }

    #[test]
    fn link_test() {
        let chain_header_a = test_chain_header();
        let entry_b = test_entry();
        let chain_header_b = ChainHeader::new(
            &entry_b.entry_type(),
            &entry_b.address(),
            &test_provenances("sig"),
            &Some(chain_header_a.address()),
            &None,
            &None,
            &test_iso_8601(),
        );
        assert_eq!(None, chain_header_a.link());
        assert_eq!(Some(chain_header_a.address()), chain_header_b.link());
    }

    #[test]
    fn entry_test() {
        assert_eq!(test_chain_header().entry_address(), &test_entry().address());
    }

    #[test]
    fn link_same_type_test() {
        let chain_header_a = test_chain_header();
        let entry_b = test_entry_b();
        let chain_header_b = ChainHeader::new(
            &entry_b.entry_type(),
            &entry_b.address(),
            &test_provenances("sig"),
            &Some(chain_header_a.address()),
            &None,
            &None,
            &test_iso_8601(),
        );
        let entry_c = test_entry_a();
        let chain_header_c = ChainHeader::new(
            &entry_c.entry_type(),
            &entry_c.address(),
            &test_provenances("sig"),
            &Some(chain_header_b.address()),
            &Some(chain_header_a.address()),
            &None,
            &test_iso_8601(),
        );

        assert_eq!(None, chain_header_a.link_same_type());
        assert_eq!(None, chain_header_b.link_same_type());
        assert_eq!(
            Some(chain_header_a.address()),
            chain_header_c.link_same_type()
        );
    }

    #[test]
    /// test header.address() against a known value
    fn known_address() {
        assert_eq!(
            test_chain_header_a().address(),
            test_chain_header().address()
        );
    }

    #[test]
    /// test that different entry content returns different addresses
    fn address_entry_content() {
        assert_ne!(
            test_chain_header_a().address(),
            test_chain_header_b().address()
        );
    }

    #[test]
    /// test that different entry types returns different addresses
    fn address_entry_type() {
        assert_ne!(
            ChainHeader::new(
                &test_entry_type_a(),
                &test_entry().address(),
                &test_provenances("sig"),
                &None,
                &None,
                &None,
                &test_iso_8601(),
            )
            .address(),
            ChainHeader::new(
                &test_entry_type_b(),
                &test_entry().address(),
                &test_provenances("sig"),
                &None,
                &None,
                &None,
                &test_iso_8601(),
            )
            .address(),
        );
    }

    #[test]
    /// test that different chain state returns different addresses
    fn address_chain_state() {
        let entry = test_entry();
        assert_ne!(
            test_chain_header().address(),
            ChainHeader::new(
                &entry.entry_type(),
                &entry.address(),
                &test_provenances("sig"),
                &Some(test_chain_header().address()),
                &None,
                &None,
                &test_iso_8601(),
            )
            .address(),
        );
    }

    #[test]
    /// test that different type_next returns different addresses
    fn address_type_next() {
        let entry = test_entry();
        assert_ne!(
            test_chain_header().address(),
            ChainHeader::new(
                &entry.entry_type(),
                &entry.address(),
                &test_provenances("sig"),
                &None,
                &Some(test_chain_header().address()),
                &None,
                &test_iso_8601(),
            )
            .address(),
        );
    }
}
