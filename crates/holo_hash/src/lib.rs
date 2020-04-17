#![deny(missing_docs)]
//! holo_hash::HoloHash is a hashing framework for Holochain.
//!
//! Note that not all HoloHashes are simple hashes of the full content as you
//! might expect in a "content-addressable" application.
//!
//! For example, the content of an AgentHash is simply the key itself to
//! enable self-proving signatures. DhtOps sometimes hash either entry content
//! or header content to produce their hashes, depending on which type
//! of operation it is.
//!
//! HoloHash implements `Display` providing a `to_string()` function accessing
//! the hash as a user friendly string. It also provides TryFrom for string
//! types allowing you to parse this string representation.
//!
//! HoloHash includes a 4 byte (or u32) dht "location" that serves dual purposes.
//!  - It is used as a checksum when parsing string representations.
//!  - It is used as a u32 in our dht sharding algorithm.
//!
//! HoloHash implements SerializedBytes to make it easy to cross ffi barriers
//! such as WASM and the UI websocket.
//!
//! # Example
//!
//! ```
//! use holo_hash::*;
//! use std::convert::TryInto;
//! use holochain_serialized_bytes::SerializedBytes;
//!
//! let entry: HoloHash =
//!     "uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
//!     .try_into()
//!     .unwrap();
//!
//! assert_eq!(3860645936, entry.get_loc());
//!
//! let bytes: SerializedBytes = entry.try_into().unwrap();
//!
//! assert_eq!(
//!     "{\"type\":\"EntryHash\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
//!     &format!("{:?}", bytes),
//! );
//! ```
//!
//! # Advanced
//!
//! Calculating hashes takes time - In a futures context we don't want to block.
//! HoloHash provides sync (blocking) and async (non-blocking) apis for hashing.
//!
//! ```
//! use holo_hash::*;
//!
//! let entry_content = b"test entry content";
//!
//! let entry_hash: HoloHash = EntryHash::with_data_sync(entry_content).into();
//!
//! // if in a futures context you should await instead to not block executor:
//! // let entry_hash = EntryHash::with_data(entry_content).await;
//!
//! assert_eq!(
//!     "EntryHash(uhCEkhPbA5vaw3Fk-ZvPSKuyyjg8eoX98fve75qiUEFgAE3BO7D4d)",
//!     &format!("{:?}", entry_hash),
//! );
//!
//! ```
//!
//! ## Sometimes your data doesn't want to be re-hashed:
//!
//! ```
//! use holo_hash::*;
//!
//! // pretend our pub key is all 0xdb bytes
//! let agent_pub_key = vec![0xdb; 32];
//!
//! let agent_id: HoloHash = AgentHash::with_pre_hashed_sync(agent_pub_key).into();
//!
//! // if in a futures context you should await instead to not block executor:
//! // let agent_id = AgentHash::with_pre_hashed(agent_pub_key).await;
//!
//! assert_eq!(
//!     "AgentHash(uhCAk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv)",
//!     &format!("{:?}", agent_id),
//! );
//!
//! ```

pub use holo_hash_core::HoloHashCoreHash;

/// Holo Hash Error Type.
#[derive(Debug, thiserror::Error)]
pub enum HoloHashError {
    /// holo hashes begin with a lower case u (base64url_no_pad)
    #[error("holo hashes begin with a lower case u (base64url_no_pad)")]
    NoU,

    /// could not base64 decode the holo hash
    #[error("could not base64 decode the holo hash")]
    BadBase64,

    /// this string is not the right size for a holo hash
    #[error("this string is not the right size for a holo hash")]
    BadSize,

    /// this hash does not seem to match a known holo hash prefix
    #[error("this hash does not seem to match a known holo hash prefix")]
    BadPrefix,

    /// checksum validation failed
    #[error("checksum validation failed")]
    BadChecksum,
}

/*

This code helps us find unregistered varints in multihash that
are at least somewhat user-friendly that we could register.

```javascript
#!/usr/bin/env node

const varint = require('varint')

for (let i = 0x00; i <= 0xff; ++i) {
  for (let j = 0x00; j <= 0xff; ++j) {
    let code
    try {
      code = varint.decode([i, j])
    } catch (e) {
      continue
    }
    if (code < 256 || varint.decode(varint.encode(code)) !== code) {
      continue
    }
    const full = Buffer.from([i, j, 0x24]).toString('base64')
    if (full[0] !== 'h' && full[0] !== 'H') {
      continue
    }
    console.log(full, varint.decode([i, j]), Buffer.from([i, j, 0x24]))
  }
}
```

hCAk 4100 <Buffer 84 20 24>
hCEk 4228 <Buffer 84 21 24>
hCIk 4356 <Buffer 84 22 24>
hCMk 4484 <Buffer 84 23 24>
hCQk 4612 <Buffer 84 24 24>
hCUk 4740 <Buffer 84 25 24>
hCYk 4868 <Buffer 84 26 24>
hCck 4996 <Buffer 84 27 24>
hCgk 5124 <Buffer 84 28 24>
hCkk 5252 <Buffer 84 29 24>
hCok 5380 <Buffer 84 2a 24>
hCsk 5508 <Buffer 84 2b 24>
hCwk 5636 <Buffer 84 2c 24>
hC0k 5764 <Buffer 84 2d 24>
hC4k 5892 <Buffer 84 2e 24>
hC8k 6020 <Buffer 84 2f 24>
*/

const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k
const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk
const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk
const ENTRY_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk
const DHTOP_PREFIX: &[u8] = &[0x84, 0x24, 0x24]; // uhCQk

/// internal compute a 32 byte blake2b hash
fn blake2b_256(data: &[u8]) -> Vec<u8> {
    let hash = blake2b_simd::Params::new().hash_length(32).hash(data);
    hash.as_bytes().to_vec()
}

/// internal compute a 16 byte blake2b hash
fn blake2b_128(data: &[u8]) -> Vec<u8> {
    let hash = blake2b_simd::Params::new().hash_length(16).hash(data);
    hash.as_bytes().to_vec()
}

/// internal compute the holo dht location u32
fn holo_dht_location_bytes(data: &[u8]) -> Vec<u8> {
    let hash = blake2b_128(data);
    let mut out = vec![hash[0], hash[1], hash[2], hash[3]];
    for i in (4..16).step_by(4) {
        out[0] ^= hash[i];
        out[1] ^= hash[i + 1];
        out[2] ^= hash[i + 2];
        out[3] ^= hash[i + 3];
    }
    out
}

/// internal REPR for holo hash
fn holo_hash_encode(prefix: &[u8], data: &[u8]) -> String {
    format!(
        "u{}{}",
        base64::encode_config(prefix, base64::URL_SAFE_NO_PAD),
        base64::encode_config(data, base64::URL_SAFE_NO_PAD),
    )
}

/// internal PARSE for holo hash REPR
fn holo_hash_decode(prefix: &[u8], s: &str) -> Result<Vec<u8>, HoloHashError> {
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let s = match base64::decode_config(&s[1..], base64::URL_SAFE_NO_PAD) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(s) => s,
    };
    if s.len() != 39 {
        return Err(HoloHashError::BadSize);
    }
    if &s[..3] != prefix {
        return Err(HoloHashError::BadPrefix);
    }
    let s = &s[3..];
    let loc_bytes = holo_dht_location_bytes(&s[..32]);
    let loc_bytes: &[u8] = &loc_bytes;
    if loc_bytes != &s[32..] {
        return Err(HoloHashError::BadChecksum);
    }
    Ok(s.to_vec())
}

/// internal parse helper for HoloHash enum.
fn holo_hash_parse(s: &str) -> Result<HoloHash, HoloHashError> {
    use std::convert::TryFrom;
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    match &s[1..5] {
        "hC0k" => Ok(HoloHash::DnaHash(DnaHash::try_from(s)?)),
        "hCIk" => Ok(HoloHash::NetIdHash(NetIdHash::try_from(s)?)),
        "hCAk" => Ok(HoloHash::AgentHash(AgentHash::try_from(s)?)),
        "hCEk" => Ok(HoloHash::EntryHash(EntryHash::try_from(s)?)),
        "hCQk" => Ok(HoloHash::DhtOpHash(DhtOpHash::try_from(s)?)),
        _ => Err(HoloHashError::BadPrefix),
    }
}

macro_rules! new_holo_hash {
    ( $( $doc:expr , $name:ident , $prefix:expr , )* ) => {
        $(
            #[doc = $doc]
            #[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
            pub struct $name(holo_hash_core::$name);

            impl $name {
                /// Construct a new hash instance from an already generated hash.
                pub fn with_pre_hashed_sync(mut hash: Vec<u8>) -> Self {
                    assert_eq!(32, hash.len(), "only 32 byte hashes supported");
                    hash.append(&mut holo_dht_location_bytes(&hash));
                    Self(holo_hash_core::$name::new(hash))
                }

                /// Construct a new hash instance from an already generated hash.
                #[cfg(feature = "async")]
                pub async fn with_pre_hashed(hash: Vec<u8>) -> Self {
                    tokio::task::block_in_place(|| {
                        $name::with_pre_hashed_sync(hash)
                    })
                }

                /// Construct a new hash instance from raw data (blocking).
                pub fn with_data_sync(data: &[u8]) -> Self {
                    $name::with_pre_hashed_sync(blake2b_256(data))
                }

                /// Construct a new hash instance from raw data.
                #[cfg(feature = "async")]
                pub async fn with_data(data: &[u8]) -> Self {
                    tokio::task::block_in_place(|| {
                        $name::with_data_sync(data)
                    })
                }
            }

            impl HoloHashCoreHash for $name {
                fn get_raw(&self) -> &[u8] {
                    self.0.get_raw()
                }

                fn get_bytes(&self) -> &[u8] {
                    self.0.get_bytes()
                }

                fn get_loc(&self) -> u32 {
                    self.0.get_loc()
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}({})", stringify!($name), holo_hash_encode($prefix, self.0.get_raw()))
                }
            }

            impl ::std::fmt::Display for $name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}", holo_hash_encode($prefix, self.0.get_raw()))
                }
            }

            impl ::std::convert::From<$name> for holo_hash_core::HoloHashCore {
                fn from(h: $name) -> Self {
                    holo_hash_core::HoloHashCore::$name(h.0)
                }
            }

            impl ::std::convert::From<$name> for HoloHash {
                fn from(h: $name) -> Self {
                    HoloHash::$name(h)
                }
            }

            impl ::std::convert::TryFrom<&str> for $name {
                type Error = HoloHashError;

                fn try_from(s: &str) -> Result<Self, Self::Error> {
                    Ok(Self(holo_hash_core::$name::new(holo_hash_decode($prefix, s.as_ref())?)))
                }
            }

            impl ::std::convert::TryFrom<&String> for $name {
                type Error = HoloHashError;

                fn try_from(s: &String) -> Result<Self, Self::Error> {
                    let s: &str = &s;
                    $name::try_from(s)
                }
            }

            impl ::std::convert::TryFrom<String> for $name {
                type Error = HoloHashError;

                fn try_from(s: String) -> Result<Self, Self::Error> {
                    $name::try_from(&s)
                }
            }
        )*

        /// An unified enum representing the holo hash types.
        #[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        #[serde(tag = "type", content = "hash")]
        pub enum HoloHash {
            $(
                #[doc = $doc]
                $name($name),
            )*
        }
        holochain_serialized_bytes::holochain_serial!(HoloHash);

        impl HoloHashCoreHash for HoloHash {
            fn get_raw(&self) -> &[u8] {
                match self {
                    $(
                        HoloHash::$name(i) => i.get_raw(),
                    )*
                }
            }

            fn get_bytes(&self) -> &[u8] {
                match self {
                    $(
                        HoloHash::$name(i) => i.get_bytes(),
                    )*
                }
            }

            fn get_loc(&self) -> u32 {
                match self {
                    $(
                        HoloHash::$name(i) => i.get_loc(),
                    )*
                }
            }
        }

        impl ::std::convert::From<HoloHash> for holo_hash_core::HoloHashCore {
            fn from(h: HoloHash) -> Self {
                match h {
                    $(
                        HoloHash::$name(i) => i.0.into(),
                    )*
                }
            }
        }

        impl std::fmt::Debug for HoloHash {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        HoloHash::$name(i) => write!(f, "{:?}", i),
                    )*
                }
            }
        }

        impl std::fmt::Display for HoloHash {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        HoloHash::$name(i) => write!(f, "{}", i),
                    )*
                }
            }
        }

        impl ::std::convert::TryFrom<&str> for HoloHash {
            type Error = HoloHashError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                holo_hash_parse(s)
            }
        }

        impl ::std::convert::TryFrom<&String> for HoloHash {
            type Error = HoloHashError;

            fn try_from(s: &String) -> Result<Self, Self::Error> {
                let s: &str = &s;
                HoloHash::try_from(s)
            }
        }

        impl ::std::convert::TryFrom<String> for HoloHash {
            type Error = HoloHashError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                HoloHash::try_from(&s)
            }
        }
    };
}

new_holo_hash! {
    "Represents a Holo/Holochain DnaHash - The hash of a specific hApp DNA. (uhC0k...)",
    DnaHash,
    DNA_PREFIX,

    "Represents a Holo/Holochain NetIdHash - Network Ids let you create hard dht network divisions. (uhCIk...)",
    NetIdHash,
    NET_ID_PREFIX,

    "Represents a Holo/Holochain AgentHash - A libsodium signature public key. (uhCAk...)",
    AgentHash,
    AGENT_PREFIX,

    "Represents a Holo/Holochain EntryHash - A direct hash of the entry data. (uhCEk...)",
    EntryHash,
    ENTRY_PREFIX,

    "Represents a Holo/Holochain DhtOpHash - The hash used is tuned by dht ops. (uhCQk...)",
    DhtOpHash,
    DHTOP_PREFIX,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn check_serialized_bytes() {
        let h: HoloHash = "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();

        let h: holochain_serialized_bytes::SerializedBytes = h.try_into().unwrap();

        assert_eq!(
            "{\"type\":\"AgentHash\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
            &format!("{:?}", h),
        );

        let h: HoloHash = h.try_into().unwrap();

        assert_eq!(
            "AgentHash(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );
    }

    #[test]
    fn holo_hash_parse() {
        let h: HoloHash = "uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "DnaHash(uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );

        let h: HoloHash = "uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "NetIdHash(uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );

        let h: HoloHash = "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "AgentHash(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );

        let h: HoloHash = "uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "EntryHash(uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );

        let h: HoloHash = "uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "DhtOpHash(uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );
    }

    #[test]
    fn agent_id_as_bytes_sync() {
        let hash = vec![0xdb; 32];
        let hash: &[u8] = &hash;
        let agent_id = AgentHash::with_pre_hashed_sync(hash.to_vec());
        assert_eq!(hash, agent_id.get_bytes());
    }

    #[cfg(feature = "async")]
    #[tokio::test(threaded_scheduler)]
    async fn agent_id_as_bytes() {
        tokio::task::spawn(async move {
            let hash = vec![0xdb; 32];
            let hash: &[u8] = &hash;
            let agent_id = AgentHash::with_pre_hashed(hash.to_vec()).await;
            assert_eq!(hash, agent_id.get_bytes());
        })
        .await
        .unwrap();
    }

    #[test]
    fn agent_id_prehash_sync_display() {
        let agent_id = AgentHash::with_pre_hashed_sync(vec![0xdb; 32]);
        assert_eq!(
            "uhCAk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv",
            &format!("{}", agent_id),
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test(threaded_scheduler)]
    async fn agent_id_prehash_display() {
        tokio::task::spawn(async move {
            let agent_id = AgentHash::with_pre_hashed(vec![0xdb; 32]).await;
            assert_eq!(
                "uhCAk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv",
                &format!("{}", agent_id),
            );
        })
        .await
        .unwrap();
    }

    #[test]
    fn agent_id_try_parse() {
        let agent_id: AgentHash = "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, agent_id.get_loc());
    }

    #[test]
    fn agent_id_sync_debug() {
        let agent_id = AgentHash::with_data_sync(&[0xdb; 32]);
        assert_eq!(
            "AgentHash(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", agent_id),
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test(threaded_scheduler)]
    async fn agent_id_debug() {
        tokio::task::spawn(async move {
            let agent_id = AgentHash::with_data(&[0xdb; 32]).await;
            assert_eq!(
                "AgentHash(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
                &format!("{:?}", agent_id),
            );
        })
        .await
        .unwrap();
    }

    #[test]
    fn agent_id_sync_display() {
        let agent_id = AgentHash::with_data_sync(&[0xdb; 32]);
        assert_eq!(
            "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm",
            &format!("{}", agent_id),
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test(threaded_scheduler)]
    async fn agent_id_display() {
        tokio::task::spawn(async move {
            let agent_id = AgentHash::with_data(&[0xdb; 32]).await;
            assert_eq!(
                "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm",
                &format!("{}", agent_id),
            );
        })
        .await
        .unwrap();
    }

    #[test]
    fn agent_id_sync_loc() {
        let agent_id = AgentHash::with_data_sync(&[0xdb; 32]);
        assert_eq!(3_860_645_936, agent_id.get_loc());
    }

    #[cfg(feature = "async")]
    #[tokio::test(threaded_scheduler)]
    async fn agent_id_loc() {
        tokio::task::spawn(async move {
            let agent_id = AgentHash::with_data(&[0xdb; 32]).await;
            assert_eq!(3_860_645_936, agent_id.get_loc());
        })
        .await
        .unwrap();
    }
}
