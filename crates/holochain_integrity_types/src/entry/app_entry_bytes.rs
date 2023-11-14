use super::EntryError;
use super::ENTRY_SIZE_LIMIT;
use holochain_serialized_bytes::prelude::*;

/// Newtype for the bytes comprising an App entry
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct AppEntryBytes(pub SerializedBytes);

impl std::fmt::Debug for AppEntryBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "AppEntryBytes({})",
            holochain_util::hex::many_bytes_string(self.0.bytes())
        ))
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
