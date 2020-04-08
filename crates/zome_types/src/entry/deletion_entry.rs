use crate::address::Address;
use crate::prelude::*;

//-------------------------------------------------------------------------------------------------
// DeletionEntry
//-------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, SerializedBytes, Eq)]
pub struct DeletionEntry {
    deleted_entry_address: Address,
}

impl DeletionEntry {
    pub fn new(deleted_entry_address: Address) -> Self {
        DeletionEntry {
            deleted_entry_address,
        }
    }

    pub fn deleted_entry_address(&self) -> &Address {
        &self.deleted_entry_address
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::address::Addressable;
    use sx_fixture::*;

    pub fn test_deletion_entry() -> DeletionEntry {
        let entry = Entry::fixture(FixtureType::A);
        DeletionEntry::new(entry.address())
    }

    #[test]
    fn deletion_entry_smoke_test() {
        assert_eq!(
            Entry::fixture(FixtureType::A).address(),
            test_deletion_entry().deleted_entry_address().clone()
        );
    }
}
