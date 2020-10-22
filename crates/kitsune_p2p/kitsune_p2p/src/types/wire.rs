//! KitsuneP2p Wire Protocol Encoding Decoding

use derive_more::*;

/// Type used for content data of wire messages.
#[derive(Debug, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize)]
pub struct WireData(#[serde(with = "serde_bytes")] pub Vec<u8>);

kitsune_p2p_types::write_codec_enum! {
    /// KitsuneP2p Wire Protocol Top-Level Enum.
    codec Wire {
        /// "Call" to the remote.
        Call(0x01) {
            data.0: WireData,
        },

        /// "Notify" the remote.
        Notify(0x02) {
            data.0: WireData,
        },
    }
}
