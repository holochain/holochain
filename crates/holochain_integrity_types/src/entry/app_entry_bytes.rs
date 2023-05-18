use super::EntryError;
use super::ENTRY_SIZE_LIMIT;
use holo_hash::bytes_to_hex;
use holochain_serialized_bytes::prelude::*;

/// Newtype for the bytes comprising an App entry
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppEntryBytes(pub SerializedBytes);

impl std::fmt::Debug for AppEntryBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "AppEntryBytes({})",
            many_bytes_string(self.0.bytes())
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

/// Helpful pattern for debug formatting many bytes.
/// If the size is > 32 bytes, only the first 8 and last 8 bytes will be displayed.
pub fn many_bytes_string(bytes: &[u8]) -> String {
    if bytes.len() <= 32 {
        format!("0x{}", bytes_to_hex(bytes, false))
    } else {
        let l = bytes.len();
        format!(
            "[0x{}..{}; len={}]",
            bytes_to_hex(&bytes[0..8], false),
            bytes_to_hex(&bytes[l - 8..l], false),
            l
        )
    }
}
