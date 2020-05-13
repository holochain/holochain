// FIXME: uncomment this deny [TK-01128]
// #![deny(missing_docs)]

pub mod conductor;
pub mod core;
pub mod fixt;
pub extern crate strum;
#[macro_use]
extern crate strum_macros;
use holochain_wasmer_host;

#[macro_export]
macro_rules! start_hard_timeout {
    () => {{
        if cfg!(test) {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards")
        } else {
            std::time::Duration::new(0, 0)
        }
    }};
}

#[macro_export]
macro_rules! end_hard_timeout {
    ( $t0:ident, $timeout:literal ) => {{
        use std::convert::TryFrom;
        if cfg!(test) {
            let hard_timeout_nanos = i128::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_nanos(),
            )
            .unwrap()
                - i128::try_from($t0.as_nanos()).unwrap();

            dbg!(hard_timeout_nanos);
            assert!(hard_timeout_nanos < $timeout, "Exceeded hard timeout!");
        }
    }};
}
