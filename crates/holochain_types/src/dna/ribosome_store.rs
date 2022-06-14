use crate::prelude::*;

/// Key for the [EntryDef] buffer
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, SerializedBytes,
)]
pub struct EntryDefBufferKey {
    /// The zome to which this entry def belongs
    pub zome: IntegrityZomeDef,
    /// The index, for ordering
    pub entry_def_position: EntryDefIndex,
}

impl EntryDefBufferKey {
    /// Create a new key
    pub fn new(zome: IntegrityZomeDef, entry_def_position: EntryDefIndex) -> Self {
        Self {
            zome,
            entry_def_position,
        }
    }
}
