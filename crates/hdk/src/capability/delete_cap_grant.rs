#[macro_export]
macro_rules! delete_cap_grant {
    ( $hash:expr ) => {{
        $crate::delete!($hash)
    }};
}
