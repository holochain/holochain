/// trivial macro to get N cryptographically random bytes
///
/// ```ignore
/// let five_bytes = random_bytes!(5)?;
/// ```
///
/// it's not possible to generate random bytes from inside the wasm guest so the data is provided
/// by the wasm host which implies operating system specific details re: randomness.
///
/// the bytes are cryptographically random in that they are unpredictable, to the quality of what
/// host environment offers and the crypto implementation within holochain.
///
/// the bytes are not "secure" though:
///
/// - there's no way to prove that a specific value was the result of random generation or not
/// - the bytes are open in memory and even (de)serialized several times between the host and guest
///
/// the bytes are not a performant or testable way to do statistical analysis (e.g. monte carlo)
/// rust provides several seedable PRNG implementations that are fast, repeatable and statistically
/// high quality even if not suitable for crypto applications
#[macro_export]
macro_rules! random_bytes {
    ( $bytes:expr ) => {{
        $crate::host_fn!(
            __random_bytes,
            $crate::prelude::RandomBytesInput::new($bytes),
            $crate::prelude::RandomBytesOutput
        )
    }};
}
