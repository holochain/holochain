//! Sometimes we have fatal errors, and need to halt the system.
//! This module provides standards for showing these messages to the user.

#[macro_export]
macro_rules! fatal {
    ($($t:tt)*) => {{
        let m = format!($($t)*);

        // human_panic is going to eat the text of our fatal error
        // so we need to duplicate it with a direct eprintln!
        eprintln!("{}", &m);

        // now panic
        panic!("{}", m);
    }};
}

#[macro_export]
macro_rules! fatal_db_deserialize_check {
    ($hint:expr, $hash_bytes:expr, $res:expr,) => {
        fatal_db_deserialize_check!($hint, $hash_bytes, $res);
    };
    ($hint:expr, $hash_bytes:expr, $res:expr) => {{
        match $res {
            Ok(res) => res,
            Err(e) => {
                $crate::fatal!(
                    r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {:?}
Deserialization Error: {:?}

We are shutting down as a precoution to prevent further corruption."#,
                    $hint,
                    $hash_bytes,
                    e,
                );
            }
        }
    }};
}

#[macro_export]
macro_rules! fatal_db_hash_check {
    ($hint:expr, $expected_bytes:expr, $actual_bytes:expr,) => {
        fatal_db_hash_check!($hint, $expected_bytes, $actual_bytes);
    };
    ($hint:expr, $expected_bytes:expr, $actual_bytes:expr) => {
        if *$expected_bytes != *$actual_bytes {
            $crate::fatal!(
                r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {:?}
Actual hash: {:?}

We are shutting down as a precaution to prevent further corruption."#,
                $hint,
                $expected_bytes,
                $actual_bytes,
            );
        }
    };
}
