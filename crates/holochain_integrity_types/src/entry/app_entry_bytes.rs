use super::EntryError;
use super::ENTRY_SIZE_LIMIT;
use holochain_serialized_bytes::prelude::*;

/// Newtype for the bytes comprising an App entry
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppEntryBytes(pub SerializedBytes);

impl std::fmt::Debug for AppEntryBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("AppEntryBytes");
        if self.0.bytes().len() <= 32 {
            t.field(&self.0).finish()
        } else {
            let z = self.0.bytes();
            let l = z.len();
            t.field(&format!(
                "[{},{},{},{},{},{},{},{},...,{},{},{},{},{},{},{},{}]",
                z[0],
                z[1],
                z[2],
                z[3],
                z[4],
                z[5],
                z[6],
                z[7],
                z[l - 1],
                z[l - 2],
                z[l - 3],
                z[l - 4],
                z[l - 5],
                z[l - 6],
                z[l - 7],
                z[l - 8],
            ))
            .finish()
        }
    }
}

impl AppEntryBytes {
    /// Get the inner SerializedBytes
    pub fn into_sb(self) -> SerializedBytes {
        self.0
    }
}

impl AsRef<SerializedBytes> for AppEntryBytes {
    fn as_ref(&self) -> &SerializedBytes {
        &self.0
    }
}

impl std::borrow::Borrow<SerializedBytes> for AppEntryBytes {
    fn borrow(&self) -> &SerializedBytes {
        &self.0
    }
}

impl std::ops::Deref for AppEntryBytes {
    type Target = SerializedBytes;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<SerializedBytes> for AppEntryBytes {
    type Error = EntryError;

    fn try_from(sb: SerializedBytes) -> Result<Self, EntryError> {
        let size = sb.bytes().len();
        if size > ENTRY_SIZE_LIMIT {
            Err(EntryError::EntryTooLarge(size))
        } else {
            Ok(Self(sb))
        }
    }
}

impl From<AppEntryBytes> for SerializedBytes {
    fn from(aeb: AppEntryBytes) -> Self {
        UnsafeBytes::from(aeb.0).into()
    }
}
