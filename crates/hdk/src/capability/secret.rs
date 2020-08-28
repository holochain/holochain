#[macro_export]
macro_rules! generate_cap_secret {
    () => {{
        random_bytes!($crate::prelude::CAP_SECRET_BYTES as u32).map(|bytes| {
            // always a fatal error if our own bytes generation
            assert_eq!($crate::prelude::CAP_SECRET_BYTES, bytes.len());
            CapSecret::from(bytes)
        })
    }};
}
