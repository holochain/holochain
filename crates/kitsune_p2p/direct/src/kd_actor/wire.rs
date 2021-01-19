use kitsune_p2p::dependencies::kitsune_p2p_types::write_codec_enum;

write_codec_enum! {
    /// Kitsune P2p Direct Wire Protocol
    codec Wire {
        /// Failure
        Failure(0x00) {
            reason.0: String,
        },

        /// Success
        Success(0x01) {
        },

        /// Message an active agent
        Message(0x10) {
            content.0: serde_json::Value,
        },
    }
}
