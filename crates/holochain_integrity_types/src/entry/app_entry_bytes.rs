use super::EntryError;
use super::ENTRY_SIZE_LIMIT;
use holochain_serialized_bytes::prelude::*;

/// Newtype for the bytes comprising an App entry
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AppEntryBytes(pub SerializedBytes);

impl std::fmt::Debug for AppEntryBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "AppEntryBytes([{}{}])",
            format!("{:?}", &self.0.bytes()[..self.0.bytes().len().min(32)])
                .trim_matches('[')
                .trim_end_matches(']'),
            if self.0.bytes().len() > 32 { ", .." } else { "" }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_entry_bytes_debug_small() {
        let bytes = AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(vec![1, 2, 3])));
        assert_eq!(format!("{:?}", bytes), "AppEntryBytes([1, 2, 3])");
    }

    #[test]
    fn app_entry_bytes_debug_boundary() {
        let bytes = AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(vec![1; 32])));
        assert_eq!(format!("{:?}", bytes), "AppEntryBytes([1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1])");
    }

    #[test]
    fn app_entry_bytes_debug_large() {
        let bytes = AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(vec![1; 60])));
        assert_eq!(format!("{:?}", bytes), "AppEntryBytes([1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, ..])");
    }
}
