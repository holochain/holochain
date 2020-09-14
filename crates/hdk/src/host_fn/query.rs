/// Walks the source chain in reverse (latest to oldest) filtering by header and/or entry type
///
/// Given a header and entry type, returns a Vec<HeaderHash>
///
/// @todo document this better with examples
/// @todo do we want to return elements rather than hashes?
/// @todo implement cap grant/claim usage in terms of query
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
