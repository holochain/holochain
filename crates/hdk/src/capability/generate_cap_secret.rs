/// Generate a cryptographically strong CapSecret on the host.
///
/// You should _always_ use this function to generate new secrets.
/// You should _always_ generate new secrets if ever in doubt about secret re-use.
///
/// Re-using secrets is forbidden across claims and grants per-source chain.
///
/// Predictable and/or short secrets represent a serious security vulnerability.
#[macro_export]
macro_rules! generate_cap_secret {
    () => {{
        random_bytes!($crate::prelude::CAP_SECRET_BYTES as u32).map(|bytes| {
            // always a fatal error if our own bytes generation has the wrong number of bytes
            assert_eq!($crate::prelude::CAP_SECRET_BYTES, bytes.len());
            CapSecret::from(bytes)
        })
    }};
}
