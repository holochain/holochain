#![deny(missing_docs)]
//! holo_hash_core::HoloHashCore is a minimal hash type used for WASM so we
//! don't bring in a lot of dependencies. You cannot generate hashes, or
//! user friendly string representations of hashes with this crate alone;
//! You must use holo_hash::HoloHash for that.
//!
//! # Example
//!
//! ```
//! use holo_hash_core::*;
//!
//! let h: HoloHashCore = DnaHash::new(vec![0xdb; 36]).into();
//!
//! assert_eq!(3688618971, h.get_loc());
//! assert_eq!(
//!     "DnaHash(0xdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb)",
//!     &format!("{:?}", h),
//! );
//! ```

/// internal convert 4 location bytes into a u32 location
fn bytes_to_loc(bytes: &[u8]) -> u32 {
    (bytes[0] as u32)
        + ((bytes[1] as u32) << 8)
        + ((bytes[2] as u32) << 16)
        + ((bytes[3] as u32) << 24)
}

/// Indicates an item that supports HoloHash accessors
pub trait HoloHashCoreHash:
    'static
    + Send
    + Sync
    + std::fmt::Debug
    + Clone
    + std::hash::Hash
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + serde::Serialize
    + serde::Deserialize<'static>
    + std::convert::Into<HoloHashCore>
{
    /// Get the full byte array including the base 32 bytes and the 4 byte loc
    fn get_raw(&self) -> &[u8];

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    fn get_bytes(&self) -> &[u8];

    /// Fetch the holo dht location for this hash
    fn get_loc(&self) -> u32;

    /// consume into the inner byte vector
    fn into_inner(self) -> Vec<u8>;
}

macro_rules! core_holo_hash {
    ( $( $doc:expr , $name:ident , )* ) => {

        $(
            #[doc = $doc]
            #[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
            pub struct $name(#[serde(with = "serde_bytes")] Vec<u8>);

            impl $name {
                /// Construct a new hash instance providing 36 bytes.
                pub fn new(bytes: Vec<u8>) -> Self {
                    if bytes.len() != 36 {
                        panic!("invalid holo_hash byte count, expected: 36, found: {}. {:?}", bytes.len(), &bytes);
                    }
                    Self(bytes)
                }
            }

            impl HoloHashCoreHash for $name {
                fn get_raw(&self) -> &[u8] {
                    &self.0
                }

                fn get_bytes(&self) -> &[u8] {
                    &self.0[..self.0.len() - 4]
                }

                fn get_loc(&self) -> u32 {
                    bytes_to_loc(&self.0[self.0.len() - 4..])
                }

                fn into_inner(self) -> Vec<u8> {
                    self.0
                }
            }

            impl ::std::convert::From<$name> for HoloHashCore {
                fn from(h: $name) -> Self {
                    HoloHashCore::$name(h)
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_fmt(format_args!("{}(0x", stringify!($name)))?;
                    for byte in &self.0 {
                        f.write_fmt(format_args!("{:02x}", byte))?;
                    }
                    f.write_fmt(format_args!(")"))?;
                    Ok(())
                }
            }
        )*

        /// An unified enum representing the holo hash types.
        #[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        #[serde(tag = "type", content = "hash")]
        pub enum HoloHashCore {
            $(
                #[doc = $doc]
                $name($name),
            )*
        }
        holochain_serialized_bytes::holochain_serial!(HoloHashCore);

        impl HoloHashCoreHash for HoloHashCore {
            fn get_raw(&self) -> &[u8] {
                match self {
                    $(
                        HoloHashCore::$name(i) => i.get_raw(),
                    )*
                }
            }

            fn get_bytes(&self) -> &[u8] {
                match self {
                    $(
                        HoloHashCore::$name(i) => i.get_bytes(),
                    )*
                }
            }

            fn get_loc(&self) -> u32 {
                match self {
                    $(
                        HoloHashCore::$name(i) => i.get_loc(),
                    )*
                }
            }

            fn into_inner(self) -> Vec<u8> {
                match self {
                    $(
                        HoloHashCore::$name(i) => i.into_inner(),
                    )*
                }
            }
        }

        impl std::fmt::Debug for HoloHashCore {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        HoloHashCore::$name(i) => write!(f, "{:?}", i),
                    )*
                }
            }
        }
    };
}

core_holo_hash! {
    "Represents a Holo/Holochain DnaHash - The hash of a specific hApp DNA. (uhC0k...)",
    DnaHash,

    "Represents a Holo/Holochain WasmHash - The hash of the wasm bytes. (uhCok...)",
    WasmHash,

    "Represents a Holo/Holochain NetIdHash - Network Ids let you create hard dht network divisions. (uhCIk...)",
    NetIdHash,

    "Represents a Holo/Holochain AgentPubKey - A libsodium signature public key. (uhCAk...)",
    AgentPubKey,

    "Represents a Holo/Holochain EntryContentHash - A direct hash of the entry data. (uhCEk...)",
    EntryContentHash,

    "Represents a Holo/Holochain HeaderHash - A direct hash of an entry header.",
    HeaderHash,

    "Represents a Holo/Holochain DhtOpHash - The hash used is tuned by dht ops. (uhCQk...)",
    DhtOpHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_type(t: &str, h: HoloHashCore) {
        assert_eq!(3_688_618_971, h.get_loc());
        assert_eq!(
            "[219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219]",
            format!("{:?}", h.get_bytes()),
        );
        assert_eq!(
            format!(
                "{}(0xdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb)",
                t
            ),
            format!("{:?}", h),
        );
    }

    #[test]
    fn test_enum_types() {
        assert_type("DnaHash", DnaHash::new(vec![0xdb; 36]).into());
        assert_type("NetIdHash", NetIdHash::new(vec![0xdb; 36]).into());
        assert_type("AgentPubKey", AgentPubKey::new(vec![0xdb; 36]).into());
        assert_type(
            "EntryContentHash",
            EntryContentHash::new(vec![0xdb; 36]).into(),
        );
        assert_type("DhtOpHash", DhtOpHash::new(vec![0xdb; 36]).into());
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::new(vec![0xdb; 35]);
    }

    #[test]
    fn test_serialized_bytes() {
        use holochain_serialized_bytes::SerializedBytes;
        use std::convert::TryInto;

        let h_orig: HoloHashCore = DnaHash::new(vec![0xdb; 36]).into();

        let h = h_orig.clone();
        let h: SerializedBytes = h.try_into().unwrap();

        assert_eq!(
            "{\"type\":\"DnaHash\",\"hash\":[219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219,219]}",
            &format!("{:?}", h),
        );

        let h: HoloHashCore = h.try_into().unwrap();
        assert_eq!(h_orig, h);
    }
}
