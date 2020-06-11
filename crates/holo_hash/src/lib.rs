#![deny(missing_docs)]
//! holo_hash::HoloHash is a hashing framework for Holochain.
//!
//! Note that not all HoloHashes are simple hashes of the full content as you
//! might expect in a "content-addressable" application.
//!
//! The main exception is AgentPubKey, which is simply the key itself to
//! enable self-proving signatures. As an exception it is also named exceptionally, i.e.
//! it doesn't end in "Hash". Another exception is DhtOps which sometimes hash either entry
//! content or header content to produce their hashes, depending on which type
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
//! # #[tokio::main]
//! # async fn main () {
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
//!     "{\"type\":\"EntryContentHash\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
//!     &format!("{:?}", bytes),
//! );
//! # }
//! ```
//!
//! # Advanced
//!
//! Calculating hashes takes time - In a futures context we don't want to block.
//! HoloHash provides sync (blocking) and async (non-blocking) apis for hashing.
//!
//! ```
//! # #[tokio::main]
//! # async fn main () {
//! use holo_hash::*;
//!
//! let entry_content = b"test entry content";
//!
//! let content_hash: HoloHash = EntryContentHash::with_data(entry_content.to_vec()).await.into();
//!
//! assert_eq!(
//!     "EntryContentHash(uhCEkhPbA5vaw3Fk-ZvPSKuyyjg8eoX98fve75qiUEFgAE3BO7D4d)",
//!     &format!("{:?}", content_hash),
//! );
//! # }
//! ```
//!
//! ## Sometimes your data doesn't want to be re-hashed:
//!
//! ```
//! # #[tokio::main]
//! # async fn main () {
//! use holo_hash::*;
//!
//! // pretend our pub key is all 0xdb bytes
//! let agent_pub_key = vec![0xdb; 32];
//!
//! let agent_id: HoloHash = AgentPubKey::with_pre_hashed(agent_pub_key).into();
//!
//! assert_eq!(
//!     "AgentPubKey(uhCAk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv)",
//!     &format!("{:?}", agent_id),
//! );
//! # }
//! ```

use fixt::prelude::*;
use futures::future::FutureExt;
pub use holo_hash_core;
pub use holo_hash_core::HoloHashCoreHash;
use holochain_serialized_bytes::prelude::*;
use must_future::MustBoxFuture;

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

hCAk 4100 <Buffer 84 20 24> // agent
hCEk 4228 <Buffer 84 21 24> // entry
hCIk 4356 <Buffer 84 22 24> // net id
hCMk 4484 <Buffer 84 23 24>
hCQk 4612 <Buffer 84 24 24> // dht op
hCUk 4740 <Buffer 84 25 24>
hCYk 4868 <Buffer 84 26 24>
hCck 4996 <Buffer 84 27 24>
hCgk 5124 <Buffer 84 28 24>
hCkk 5252 <Buffer 84 29 24> // entry header
hCok 5380 <Buffer 84 2a 24> // wasm
hCsk 5508 <Buffer 84 2b 24>
hCwk 5636 <Buffer 84 2c 24>
hC0k 5764 <Buffer 84 2d 24> // dna
hC4k 5892 <Buffer 84 2e 24>
hC8k 6020 <Buffer 84 2f 24>
*/

const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k
const WASM_PREFIX: &[u8] = &[0x84, 0x2a, 0x24]; // uhCok
const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk
const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk
const ENTRY_CONTENT_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk
const HEADER_PREFIX: &[u8] = &[0x84, 0x29, 0x24]; // uhCkk
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
    // Assert the data size is relatively small so we are
    // comfortable executing this synchronously / blocking tokio thread.
    assert_eq!(32, data.len(), "only 32 byte hashes supported");

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
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    match &s[1..5] {
        "hC0k" => Ok(HoloHash::DnaHash(DnaHash::try_from(s)?)),
        "hCok" => Ok(HoloHash::WasmHash(WasmHash::try_from(s)?)),
        "hCIk" => Ok(HoloHash::NetIdHash(NetIdHash::try_from(s)?)),
        "hCAk" => Ok(HoloHash::AgentPubKey(AgentPubKey::try_from(s)?)),
        "hCEk" => Ok(HoloHash::EntryContentHash(EntryContentHash::try_from(s)?)),
        "hCQk" => Ok(HoloHash::DhtOpHash(DhtOpHash::try_from(s)?)),
        "hCkk" => Ok(HoloHash::HeaderHash(HeaderHash::try_from(s)?)),
        _ => Err(HoloHashError::BadPrefix),
    }
}

/// Common methods for all HoloHash base hash types
pub trait HoloHashBaseExt: Sized {
    /// Construct a new hash instance from an already generated hash.
    fn with_pre_hashed(hash: Vec<u8>) -> Self;
}

/// Common methods for all HoloHash hash types
pub trait HoloHashExt: HoloHashBaseExt + Sized {
    /// Construct a new hash instance from raw data.
    fn with_data(data: Vec<u8>) -> MustBoxFuture<'static, Self>;
}

macro_rules! new_holo_hash {
    ( $( $doc:expr , $name:ident , $prefix:expr , )* ) => {
        $(
            #[doc = $doc]
            #[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
            pub struct $name(holo_hash_core::$name);

            impl HoloHashBaseExt for $name {
                /// Construct a new hash instance from an already generated hash.
                fn with_pre_hashed(mut hash: Vec<u8>) -> Self {
                    // Assert the data size is relatively small so we are
                    // comfortable executing this synchronously / blocking
                    // tokio thread.
                    assert_eq!(32, hash.len(), "only 32 byte hashes supported");

                    hash.append(&mut holo_dht_location_bytes(&hash));
                    Self(holo_hash_core::$name::new(hash))
                }
            }

            impl HoloHashExt for $name {

                /// Construct a new hash instance from raw data.
                fn with_data(data: Vec<u8>) -> MustBoxFuture<'static, Self> {
                    async move {
                        tokio::task::spawn_blocking(move || {
                            use $crate::HoloHashBaseExt;
                            $name::with_pre_hashed(blake2b_256(&data))
                        }).await.expect("spawn_blocking thread panic")
                    }.boxed().into()
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

                fn into_inner(self) -> Vec<u8> {
                    self.0.into_inner()
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

            impl From<::holo_hash_core::$name> for $crate::$name {
                fn from(core_hash: ::holo_hash_core::$name) -> Self {
                    crate::$name(core_hash)
                }
            }

            impl From<$crate::$name> for ::holo_hash_core::$name {
                fn from(hash: $crate::$name) -> ::holo_hash_core::$name {
                    ::holo_hash_core::$name::new(hash.into_inner())
                }
            }

            impl AsRef<[u8]> for $crate::$name {
                fn as_ref(&self) -> &[u8] {
                    self.get_bytes()
                }
            }

            fixturator!(
                $name,
                {
                    $crate::$name::with_pre_hashed(vec![0; 32])
                },
                {
                    let mut random_bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
                    $crate::$name::with_pre_hashed(random_bytes)
                },
                {
                    let ret = $crate::$name::with_pre_hashed(vec![self.0.index as _; 32]);
                    self.0.index = (self.0.index as u8).wrapping_add(1) as usize;
                    ret
                }
            );
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

            fn into_inner(self) -> Vec<u8> {
                match self {
                    $(
                        HoloHash::$name(i) => i.into_inner(),
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

        impl ::std::convert::AsRef<[u8]> for HoloHash {
            fn as_ref(&self) -> &[u8] {
                self.get_bytes()
            }
        }
    };
}

new_holo_hash! {
    "Represents a Holo/Holochain DnaHash - The hash of a specific hApp DNA. (uhC0k...)",
    DnaHash,
    DNA_PREFIX,

    "Represents a Holo/Holochain WasmHash - The hash of the wasm bytes. (uhCok...)",
    WasmHash,
    WASM_PREFIX,

    "Represents a Holo/Holochain NetIdHash - Network Ids let you create hard dht network divisions. (uhCIk...)",
    NetIdHash,
    NET_ID_PREFIX,

    "Represents a Holo/Holochain AgentPubKey - A libsodium signature public key. (uhCAk...)",
    AgentPubKey,
    AGENT_PREFIX,

    "Represents a Holo/Holochain EntryContentHash - A direct hash of the entry content. (uhCEk...)",
    EntryContentHash,
    ENTRY_CONTENT_PREFIX,

    "Represents a Holo/Holochain HeaderHash - A direct hash of the entry header. (uhCkk...)",
    HeaderHash,
    HEADER_PREFIX,

    "Represents a Holo/Holochain DhtOpHash - The hash used is tuned by dht ops. (uhCQk...)",
    DhtOpHash,
    DHTOP_PREFIX,
}

mod hashed;
pub use hashed::*;

#[macro_use]
mod make_hashed_macro;

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    /// test struct
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
    pub struct MyTest {
        /// string
        pub s: String,
        /// integer
        pub i: i64,
    }

    make_hashed! {
        Visibility(pub),
        HashedName(MyTestHashed),
        ContentType(MyTest),
        HashType(DhtOpHash),
    }

    #[tokio::test(threaded_scheduler)]
    async fn check_hashed_type() {
        use crate::hashed::Hashed;

        let my_type = MyTest {
            s: "test".to_string(),
            i: 42,
        };

        let my_type_hashed = MyTestHashed::with_data(my_type).await.unwrap();

        assert_eq!(
            "uhCQkQFRMcbVVfPJ5AbAv0HJq0geatTakGEEj5rpv_Dp0pjmJob3P",
            my_type_hashed.as_hash().to_string(),
        );
    }

    #[test]
    fn check_serialized_bytes() {
        let h: HoloHash = "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();

        let h: holochain_serialized_bytes::SerializedBytes = h.try_into().unwrap();

        assert_eq!(
            "{\"type\":\"AgentPubKey\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
            &format!("{:?}", h),
        );

        let h: HoloHash = h.try_into().unwrap();

        assert_eq!(
            "AgentPubKey(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
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
            "AgentPubKey(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
            &format!("{:?}", h),
        );

        let h: HoloHash = "uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, h.get_loc());
        assert_eq!(
            "EntryContentHash(uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
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

    #[tokio::test(threaded_scheduler)]
    async fn agent_id_as_bytes() {
        tokio::task::spawn(async move {
            let hash = vec![0xdb; 32];
            let hash: &[u8] = &hash;
            let agent_id = AgentPubKey::with_pre_hashed(hash.to_vec());
            assert_eq!(hash, agent_id.get_bytes());
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn agent_id_prehash_display() {
        tokio::task::spawn(async move {
            let agent_id = AgentPubKey::with_pre_hashed(vec![0xdb; 32]);
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
        let agent_id: AgentPubKey = "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
            .try_into()
            .unwrap();
        assert_eq!(3_860_645_936, agent_id.get_loc());
    }

    #[tokio::test(threaded_scheduler)]
    async fn agent_id_debug() {
        tokio::task::spawn(async move {
            let agent_id = AgentPubKey::with_data(vec![0xdb; 32]).await;
            assert_eq!(
                "AgentPubKey(uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
                &format!("{:?}", agent_id),
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn agent_id_display() {
        tokio::task::spawn(async move {
            let agent_id = AgentPubKey::with_data(vec![0xdb; 32]).await;
            assert_eq!(
                "uhCAkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm",
                &format!("{}", agent_id),
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn agent_id_loc() {
        tokio::task::spawn(async move {
            let agent_id = AgentPubKey::with_data(vec![0xdb; 32]).await;
            assert_eq!(3_860_645_936, agent_id.get_loc());
        })
        .await
        .unwrap();
    }
}
