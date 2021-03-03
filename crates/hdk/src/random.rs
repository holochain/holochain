use crate::prelude::*;

/// Get N cryptographically strong random bytes.
///
/// ```ignore
/// let five_bytes = random_bytes(5)?;
/// ```
///
/// It's not possible to generate random bytes from inside the wasm guest so the data is provided
/// by the wasm host which implies operating system specific details re: randomness.
///
/// The bytes are cryptographically random in that they are unpredictable, to the quality of what
/// host environment offers and the crypto implementation within holochain.
///
/// The bytes are not "secure" though:
///
/// - there's no way to prove that a specific value was the result of random generation or not
/// - the bytes are open in memory and even (de)serialized several times between the host and guest
///
/// The bytes are not a performant or testable way to do statistical analysis (e.g. monte carlo).
/// Rust provides several seedable PRNG implementations that are fast, repeatable and statistically
/// high quality even if not suitable for crypto applications. If you need to do anything with
/// statistics it is usually recommended to generate or provide a seed and then use an appropriate
/// PRNG from there.
///
/// See the rand rust crate
pub fn random_bytes(number_of_bytes: u32) -> ExternResult<Bytes> {
    HDK.get()
        .ok_or(WasmError::Guest(HDK_NOT_REGISTERED.to_string()))?
        .random_bytes(number_of_bytes)
}

pub trait TryFromRandom {
    fn try_from_random() -> ExternResult<Self>
    where
        Self: Sized;
}

/// Ideally we wouldn't need to do this with a macro.
/// All we want is to implement this trait with whatever length our random-bytes-new-types need to
/// be, but if we use a const on the trait directly we get 'constant expression depends on a
/// generic parameter'
macro_rules! impl_try_from_random {
    ( $t:ty, $bytes:expr ) => {
        impl TryFromRandom for $t {
            fn try_from_random() -> $crate::prelude::ExternResult<Self> {
                $crate::prelude::random_bytes($bytes as u32).map(|bytes| {
                    // Always a fatal error if our own bytes generation has the wrong length.
                    assert_eq!($bytes, bytes.len());
                    let mut inner = [0; $bytes];
                    inner.copy_from_slice(bytes.as_ref());
                    Self::from(inner)
                })
            }
        }
    };
}

impl_try_from_random!(
    CapSecret,
    holochain_zome_types::capability::CAP_SECRET_BYTES
);

// @todo don't generate these in wasm.
// What we really want to be doing is have secrets generated in lair and then lair passes back an
// opaque reference to the secret.
// That is why the struct is is called KeyRef not Key.
impl_try_from_random!(
    SecretBoxKeyRef,
    holochain_zome_types::x_salsa20_poly1305::key_ref::KEY_REF_BYTES
);
