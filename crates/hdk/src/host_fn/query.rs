#[macro_export]
macro_rules! query {
    ( $base:expr ) => {{
        $crate::host_fn!(
            __query,
            $crate::prelude::QueryInput::new($base),
            $crate::prelude::QueryOutput
        )
    }};
}
