//! Holochain Zome Types: only the types needed by Holochain application
//! developers to use in their Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm. In contrast, the
//! [holochain_types crate](https://crates.io/crates/holochain_types)
//! contains more types which are used by Holochain itself.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod agent_info;
pub mod bytes;
#[allow(missing_docs)]
pub mod call;
#[allow(missing_docs)]
pub mod call_remote;
pub mod capability;
pub mod cell;
#[allow(missing_docs)]
pub mod crdt;
pub mod debug;
pub mod element;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
#[allow(missing_docs)]
pub mod header;
#[allow(missing_docs)]
pub mod init;
#[allow(missing_docs)]
pub mod link;
pub mod metadata;
#[allow(missing_docs)]
pub mod migrate_agent;
#[allow(missing_docs)]
pub mod post_commit;
pub mod query;
pub mod request;
pub mod signal;
pub mod signature;
pub mod timestamp;
#[allow(missing_docs)]
pub mod validate;
#[allow(missing_docs)]
pub mod validate_link;
pub mod warrant;
#[allow(missing_docs)]
pub mod xsalsa20_poly1305;
#[allow(missing_docs)]
pub mod zome;
#[allow(missing_docs)]
pub mod zome_info;
#[allow(missing_docs)]
pub mod zome_io;

#[allow(missing_docs)]
#[cfg(feature = "fixturators")]
pub mod fixt;

pub mod test_utils;

pub use entry::Entry;
pub use header::Header;
use holochain_serialized_bytes::prelude::*;
pub use zome_io::*;

#[allow(missing_docs)]
pub trait CallbackResult {
    /// if a callback result is definitive we should halt any further iterations over remaining
    /// calls e.g. over sparse names or subsequent zomes
    /// typically a clear failure is definitive but success and missing dependencies are not
    /// in the case of success or missing deps, a subsequent callback could give us a definitive
    /// answer like a fail, and we don't want to over-optimise wasm calls and miss a clear failure
    fn is_definitive(&self) -> bool;
}

#[macro_export]
/// Serialization for fixed arrays is generally not available in a way that can be derived.
/// Being able to wrap fixed size arrays is important e.g. for crypto safety etc. so this is a
/// simple way to implement serialization so that we can send these types between the host/guest.
macro_rules! fixed_array_serialization {
    ($t:ty, $len:expr) => {
        impl serde::ser::Serialize for $t {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_bytes(&self.0)
            }
        }

        impl<'de> serde::de::Deserialize<'de> for $t {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                use serde::de::Error;
                let bytes: &[u8] = serde::de::Deserialize::deserialize(deserializer)?;
                if bytes.len() != $len {
                    let exp_msg = format!("expected {} bytes got: {} bytes", $len, bytes.len());
                    return Err(D::Error::invalid_value(
                        serde::de::Unexpected::Bytes(bytes),
                        &exp_msg.as_str(),
                    ));
                }
                let mut inner: [u8; $len] = [0; $len];
                inner.clone_from_slice(bytes);
                Ok(Self(inner))
            }
        }
    };
}

#[macro_export]
/// Cryptographic secrets are fiddly at the best of times.
///
/// In wasm it is somewhat impossible to have true secrets because wasm memory is not secure.
///
///  - The host can always read wasm memory so any vulnerability in the host compromises the guest.
///  - The host/rust generally doesn't guarantee to immediately wipe/zero out freed memory, either
///    when a zome call is running or after a wasm instance is thrown away.
///
/// Most of the time we should just try to minimise the interaction between wasm and secret data.
///
/// For example, lair keeps all our private keys internal and we can only send it signing requests
/// associated with public keys.
///
/// In other contexts it is more difficult, such as when generating secrets from raw cryptographic
/// random bytes and sending them to peers directly.
///
/// The best we can do here is try to protect ourselves against third parties across the network.
/// e.g. We don't want other machines to simply `remote_call` a successful timing attack.
///
/// MITM attacks are mitigated by the networking implementation itself.
///
/// @todo given how impossible it is for wasm to protect its memory from the host, it would make
/// more sense to:
///
///  - use key exchange protocols like libsodium kx https://libsodium.gitbook.io/doc/key_exchange.
///  - keep secrets inside lair with all algorithms behind an API, wasm only has access to opaque
///    references to the secret data.
///
/// @todo implement explicit zeroing of memory for sensitive data.
macro_rules! crypto_secret {
    ($t:ty, $len:expr) => {
        $crate::fixed_array_serialization!($t, $len);

        /// Constant time equality check.
        /// This mitigates timing attacks where a remote agent can reverse engineer data by
        /// measuring tiny changes in latency associated with optimised equality checks.
        /// More matching bytes = more latency = vulnerability.
        /// This type of attack has been successfully demonstrated over a network despite varied latencies.
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                use subtle::ConstantTimeEq;
                self.0.ct_eq(&other.0).into()
            }
        }

        impl Eq for $t {}

        /// The only meaningful debug information for a cryptograhpic secret is the literal bytes.
        /// Also, encodings like base64 are not constant time so debugging could open some weird
        /// side channel issue trying to be 'human friendly'.
        /// It seems better to never try to encode secrets.
        /// @see https://docs.rs/subtle-encoding/0.5.1/subtle_encoding/
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0.to_vec(), f)
            }
        }

        /// Trivial new type derivation.
        /// Secrets should have private interiors and be constructed directly from fixed length
        /// arrays of known length.
        impl From<[u8; $len]> for $t {
            fn from(b: [u8; $len]) -> Self {
                Self(b)
            }
        }

        impl AsRef<[u8]> for $t {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }
    };
}
