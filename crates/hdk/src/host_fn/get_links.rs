/// returns all links that reference a base entry hash, optionally filtered by tag
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
/// this is mostly identical to get_link_details but returns only adds that have not been removed
/// c.f. get_link_details that returns all the adds and all the removes together
///
/// @see get_link_details
#[macro_export]
macro_rules! get_links {
    ( $base:expr ) => {
        $crate::get_links!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __get_links,
            $crate::prelude::GetLinksInput::new(($base, $tag.into())),
            $crate::prelude::GetLinksOutput
        )
    }};
}
