/// returns all link adds and removes that reference a base entry hash, optionally filtered by tag
///
/// tag filtering is a simple bytes prefix
///
/// e.g. if you had a links:
///      - a: `[ 1, 2, 3]`
///      - b: `[ 1, 2, 4]`
///      - c: `[ 1, 3, 5]`
///
/// then tag filters:
///      - `[ 1 ]` returns `[ a, b, c]`
///      - `[ 1, 2 ]` returns `[ a, b ]`
///      - `[ 1, 2, 3 ]` returns `[ a ]`
///      - `[ 5 ]` returns `[ ]` (does _not_ return c because the filter is prefix not contains)
///
/// this is mostly identical to get_links but it returns all the adds and all the removes
/// c.f. get_links that returns all the adds that have not been removed
///
/// @see get_links
#[macro_export]
macro_rules! get_link_details {
    ( $base:expr ) => {
        $crate::get_link_details!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __get_link_details,
            GetLinkDetailsInput::new(($base, $tag.into())),
            GetLinkDetailsOutput
        )
    }};
}
