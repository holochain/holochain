pub use crate::integrity::*;
use hdk::prelude::*;

use super::EntryZomes::*;

impl std::ops::Add for CounTree {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl CounTree {
    #[allow(clippy::new_ret_no_self)]
    /// ensures that a default countree exists and returns the action
    pub fn new() -> ExternResult<ActionHash> {
        Self::ensure(Self::default())
    }

    /// commits if not exists else returns found action
    /// produces redundant actions in a partition
    pub fn ensure(countree: CounTree) -> ExternResult<ActionHash> {
        match get(hash_entry(&countree)?, GetOptions::latest())? {
            Some(commit) => Ok(commit.action_address().to_owned()),
            None => create_entry(&IntegrityCrud(EntryTypes::Countree(countree))),
        }
    }

    pub fn action_details(action_hashes: Vec<ActionHash>) -> ExternResult<Vec<Option<Details>>> {
        HDK.with(|h| {
            h.borrow().get_details(
                action_hashes
                    .into_iter()
                    .map(|action_hash| GetInput::new(action_hash.into(), GetOptions::latest()))
                    .collect(),
            )
        })
    }

    /// return the Option<Details> for the entry hash from the action
    pub fn entry_details(entry_hashes: Vec<EntryHash>) -> ExternResult<Vec<Option<Details>>> {
        HDK.with(|h| {
            h.borrow().get_details(
                entry_hashes
                    .into_iter()
                    .map(|entry_hash| GetInput::new(entry_hash.into(), GetOptions::latest()))
                    .collect(),
            )
        })
    }

    /// increments the given action hash by 1 or creates it if not found
    /// this is silly as being offline resets the counter >.<
    pub fn incsert(action_hash: ActionHash) -> ExternResult<ActionHash> {
        let current: CounTree = match get(action_hash.clone(), GetOptions::latest())? {
            Some(commit) => match commit
                .entry()
                .to_app_option()
                .map_err(|e| wasm_error!(e.into()))?
            {
                Some(v) => v,
                None => return Self::new(),
            },
            None => return Self::new(),
        };

        update_entry(action_hash, &(current + CounTree(1)))
    }

    pub fn dec(action_hash: ActionHash) -> ExternResult<ActionHash> {
        delete_entry(action_hash)
    }
}
