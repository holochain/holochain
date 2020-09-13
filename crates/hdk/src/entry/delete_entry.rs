#[macro_export]
macro_rules! delete_entry {
    ( $hash:expr ) => {{
        delete!($hash)
    }};
}
