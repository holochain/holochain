//! Op metadata that can be passed through Kitsune2
//!
//! Kitsune2's `publish_ops` / `process_incoming_ops` carry opaque,
//! host-defined `Option<Bytes>` metadata with each op. Holochain
//! encodes that as a single `u8` bitmask where each bit is a
//! flag.

use bytes::Bytes;

/// Bit 0: the publisher is requesting a validation receipt for this op.
const VALIDATION_RECEIPT_REQUIRED: u8 = 0b00000001;

/// Encode the publish metadata bitmask.
///
/// Returns `None` when no flags are set, so ops that carry no metadata
/// stay cheap on the wire and decode to no set flags.
pub(crate) fn encode_publish_metadata(require_receipt: bool) -> Option<Bytes> {
    let mut flags = 0u8;
    if require_receipt {
        flags |= VALIDATION_RECEIPT_REQUIRED;
    }
    if flags == 0 {
        None
    } else {
        Some(Bytes::copy_from_slice(&[flags]))
    }
}

/// Decode whether a validation receipt is required.
///
/// Missing or empty metadata means "not required".
pub(crate) fn get_require_validation_receipt_from_metadata(metadata: &Option<Bytes>) -> bool {
    match metadata {
        Some(bytes) if !bytes.is_empty() => bytes[0] & VALIDATION_RECEIPT_REQUIRED != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_required_roundtrips() {
        let encoded = encode_publish_metadata(true);
        assert!(encoded.is_some());
        assert!(get_require_validation_receipt_from_metadata(&encoded));
    }

    #[test]
    fn no_flags_encodes_to_none() {
        let encoded = encode_publish_metadata(false);
        assert!(encoded.is_none());
        assert!(!get_require_validation_receipt_from_metadata(&encoded));
    }

    #[test]
    fn missing_or_empty_metadata_means_not_required() {
        assert!(!get_require_validation_receipt_from_metadata(&None));
        assert!(!get_require_validation_receipt_from_metadata(&Some(
            Bytes::new()
        )));
    }

    #[test]
    fn unrelated_bits_do_not_set_receipt() {
        // A byte with only higher bits set must not read as receipt-required.
        assert!(!get_require_validation_receipt_from_metadata(&Some(
            Bytes::copy_from_slice(&[0b1111_1110])
        )));
    }
}
