use crate::{HashType, HoloHash};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError, UnsafeBytes};

impl<T: HashType> std::convert::TryFrom<&HoloHash<T>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: &HoloHash<T>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        match rmp_serde::to_vec_named(t) {
            Ok(v) => Ok(SerializedBytes::from(UnsafeBytes::from(v))),
            Err(e) => Err(SerializedBytesError::ToBytes(e.to_string())),
        }
    }
}

impl<T: HashType> std::convert::TryFrom<HoloHash<T>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: HoloHash<T>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        SerializedBytes::try_from(&t)
    }
}

impl<T: HashType> std::convert::TryFrom<SerializedBytes> for HoloHash<T> {
    type Error = SerializedBytesError;
    fn try_from(sb: SerializedBytes) -> std::result::Result<HoloHash<T>, SerializedBytesError> {
        match rmp_serde::from_read_ref(sb.bytes()) {
            Ok(v) => Ok(v),
            Err(e) => Err(SerializedBytesError::FromBytes(e.to_string())),
        }
    }
}
