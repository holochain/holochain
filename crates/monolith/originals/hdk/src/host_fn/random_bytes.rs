use crate::hdk3::prelude::*;

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
/// @see the rand rust crate
pub fn random_bytes(number_of_bytes: u32) -> HdkResult<Vec<u8>> {
    Ok(host_call::<RandomBytesInput, RandomBytesOutput>(
        __random_bytes,
        &RandomBytesInput::new(number_of_bytes),
    )?
    .into_inner()
    .into_vec())
}
