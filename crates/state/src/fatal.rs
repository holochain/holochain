//! Sometimes we have fatal errors, and need to halt the system.
//! This module provides standards for showing these messages to the user.

/// Macro for standard handling of fatal errors
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

/// Macro for standard handling of db deserialization fatal errors
#[macro_export]
macro_rules! fatal_db_hash_construction_check {
    ($hint:expr, $hash:expr, $res:expr,) => {
        fatal_db_hash_construction_check!($hint, $hash, $res);
    };
    ($hint:expr, $hash:expr, $res:expr) => {{
        match $res {
            Ok(res) => res,
            Err(e) => {
                $crate::fatal!(
                    r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {:?}
Deserialization Error: {:?}

We are shutting down as a precaution to prevent further corruption."#,
                    $hint,
                    $hash,
                    e,
                );
            }
        }
    }};
}

/// Macro for standard handling of db hash integrity check failures
#[macro_export]
macro_rules! fatal_db_hash_integrity_check {
    ($hint:expr, $expected_hash:expr, $actual_hash:expr, $content:expr $(,)?) => {
        if *$expected_hash != *$actual_hash {
            $crate::fatal!(
                r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {:?}
Actual hash: {:?}
Content: {:?}

We are shutting down as a precaution to prevent further corruption."#,
                $hint,
                $expected_hash,
                $actual_hash,
                $content,
            );
        }
    };
}
