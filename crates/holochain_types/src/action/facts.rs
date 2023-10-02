use super::*;

impl NewEntryAction {
    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            Self::Create(Create { ref mut author, .. }) => author,
            Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    pub fn timestamp_mut(&mut self) -> &mut Timestamp {
        match self {
            Self::Create(Create {
                ref mut timestamp, ..
            }) => timestamp,
            Self::Update(Update {
                ref mut timestamp, ..
            }) => timestamp,
        }
    }

    pub fn action_seq_mut(&mut self) -> &mut u32 {
        match self {
            Self::Create(Create {
                ref mut action_seq, ..
            }) => action_seq,
            Self::Update(Update {
                ref mut action_seq, ..
            }) => action_seq,
        }
    }

    pub fn entry_data_mut(&mut self) -> (&mut EntryHash, &mut EntryType) {
        match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
        }
    }

    pub fn entry_hash_mut(&mut self) -> &mut EntryHash {
        self.entry_data_mut().0
    }
}
