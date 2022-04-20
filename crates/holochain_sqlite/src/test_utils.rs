//! Helpers for unit tests

#[macro_export]
/// Macro to generate a fresh reader from an DbRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! fresh_reader_test {
    ($env: expr, $f: expr) => {{
        let mut conn = $env.conn().unwrap();
        conn.with_reader_test($f)
    }};
}

#[macro_export]
/// Macro to generate a fresh reader from an DbRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! print_stmts_test {
    ($env: expr, $f: expr) => {{
        let mut conn = $env.conn().unwrap();
        conn.trace(Some(|s| println!("{}", s)));
        let r = conn.with_reader_test($f);
        conn.trace(None);
        r
    }};
}
