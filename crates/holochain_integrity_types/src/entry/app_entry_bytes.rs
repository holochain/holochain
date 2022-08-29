use super::EntryError;
use super::ENTRY_SIZE_LIMIT;
use holochain_serialized_bytes::prelude::*;

/// Newtype for the bytes comprising an App entry
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppEntryBytes(pub SerializedBytes);

impl std::fmt::Debug for AppEntryBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_many_bytes("AppEntryBytes", f, self.0.bytes())
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
/// If the size is > 32 bytes, only the first 10 and last 10 bytes will be displayed.
pub fn fmt_many_bytes(
    name: &str,
    f: &mut std::fmt::Formatter<'_>,
    bytes: &[u8],
) -> std::fmt::Result {
    if bytes.len() <= 32 {
        let mut t = f.debug_tuple(name);
        t.field(&bytes).finish()
    } else {
        let mut t = f.debug_struct(name);
        let l = bytes.len();
        t.field("length", &l);
        t.field(
            "bytes",
            &format!(
                "[{},{},{},{},{},{},{},{},...,{},{},{},{},{},{},{},{}]",
                bytes[0],
                bytes[1],
                bytes[2],
                bytes[3],
                bytes[4],
                bytes[5],
                bytes[6],
                bytes[7],
                bytes[l - 1],
                bytes[l - 2],
                bytes[l - 3],
                bytes[l - 4],
                bytes[l - 5],
                bytes[l - 6],
                bytes[l - 7],
                bytes[l - 8],
            ),
        )
        .finish()
    }
}
