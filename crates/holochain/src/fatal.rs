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
    ($hint:expr, $h:expr, $res:expr,) => {
        fatal_db_deserialize_check!($hint, $h, $res);
    };
    ($hint:expr, $h:expr, $res:expr) => {{
        match $res {
            Ok(res) => res,
            Err(e) => {
                $crate::fatal!(
                    r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {}
Deserialization Error: {:?}

We are shutting down as a precoution to prevent further corruption."#,
                    $hint,
                    $h,
                    e,
                );
            }
        }
    }};
}

#[macro_export]
macro_rules! fatal_db_hash_check {
    ($hint:expr, $h1:expr, $h2:expr,) => {
        fatal_db_hash_check!($hint, $h1, $h2);
    };
    ($hint:expr, $h1:expr, $h2:expr) => {
        if $h1 != $h2 {
            $crate::fatal!(
                r#"Holochain detected database corruption.

Corrupt module: {}
Expected hash: {}
Actual hash: {}

We are shutting down as a precaution to prevent further corruption."#,
                $hint,
                $h1,
                $h2,
            );
        }
    };
}
