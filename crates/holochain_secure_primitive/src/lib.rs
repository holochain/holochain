pub use paste;
pub use serde;
pub use subtle;

mod types;
pub use types::*;

#[macro_export]
/// Serialization for fixed arrays is generally not available in a way that can be derived.
/// Being able to wrap fixed size arrays is important e.g. for crypto safety etc. so this is a
/// simple way to implement serialization so that we can send these types between the host/guest.
macro_rules! fixed_array_serialization {
    ($t:ty, $len:expr) => {
        $crate::paste::paste! {
            impl $crate::serde::ser::Serialize for $t {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: $crate::serde::ser::Serializer,
                {
                    serializer.serialize_bytes(&self.0)
                }
            }

            struct [<Visitor$t>];

            impl<'de> $crate::serde::de::Visitor<'de> for [<Visitor$t>] {
                type Value = [u8; $len];

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str(format!("a byte array of length {}", $len).as_str())
                }

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                where
                    E: $crate::serde::de::Error,
                {
                    if value.len() == $len {
                        let mut bytes = [0 as u8; $len];
                        bytes.clone_from_slice(value);
                        Ok(bytes)
                    } else {
                        let error_message = format!("{} bytes, got {} bytes", $len, value.len());
                        Err(E::invalid_value(
                            $crate::serde::de::Unexpected::Bytes(value),
                            &error_message.as_str(),
                        ))
                    }
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: $crate::serde::de::SeqAccess<'de>,
                {
                    let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));

                    while let Some(b) = seq.next_element()? {
                        vec.push(b);
                    }

                    self.visit_bytes(&vec)
                }
            }

            impl<'de> $crate::serde::de::Deserialize<'de> for $t {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: $crate::serde::de::Deserializer<'de>,
                {
                    let bytes = deserializer.deserialize_bytes([<Visitor$t>])?;
                    Ok(Self(bytes))
                }
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
///  - use key exchange protocols like libsodium kx <https://libsodium.gitbook.io/doc/key_exchange>.
///  - keep secrets inside lair with all algorithms behind an API, wasm only has access to opaque
///    references to the secret data.
///
/// @todo implement explicit zeroing, moving and copying of memory for sensitive data.
///       - e.g. the secrecy crate <https://crates.io/crates/secrecy>
macro_rules! secure_primitive {
    ($t:ty, $len:expr) => {
        $crate::fixed_array_serialization!($t, $len);

        /// Constant time equality check.
        /// This mitigates timing attacks where a remote agent can reverse engineer data by
        /// measuring tiny changes in latency associated with optimised equality checks.
        /// More matching bytes = more latency = vulnerability.
        /// This type of attack has been successfully demonstrated over a network despite varied latencies.
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                use $crate::subtle::ConstantTimeEq;
                self.0.ct_eq(&other.0).into()
            }
        }

        impl Eq for $t {}

        /// The only meaningful debug information for a cryptograhpic secret is the literal bytes.
        /// Also, encodings like base64 are not constant time so debugging could open some weird
        /// side channel issue trying to be 'human friendly'.
        /// It seems better to never try to encode secrets.
        ///
        /// Note that when using this crate with feature "subtle-encoding", a hex
        /// representation will be used.
        //
        // @todo maybe we want something like **HIDDEN** by default and putting the actual bytes
        //       behind a feature flag?
        #[cfg(not(feature = "subtle-encoding"))]
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0.to_vec(), f)
            }
        }

        #[cfg(feature = "subtle-encoding")]
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let str = String::from_utf8(subtle_encoding::hex::encode(self.0.to_vec()))
                    .unwrap_or_else(|_| "<unparseable signature>".into());
                f.write_str(&str)
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

        impl core::convert::TryFrom<&[u8]> for $t {
            type Error = $crate::SecurePrimitiveError;
            fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
                if slice.len() == $len {
                    let mut inner = [0; $len];
                    inner.copy_from_slice(slice);
                    Ok(inner.into())
                } else {
                    Err($crate::SecurePrimitiveError::BadSize)
                }
            }
        }

        impl core::convert::TryFrom<Vec<u8>> for $t {
            type Error = $crate::SecurePrimitiveError;
            fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
                Self::try_from(v.as_ref())
            }
        }

        impl AsRef<[u8]> for $t {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }
    };
}
