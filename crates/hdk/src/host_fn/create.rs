/// General macro that can create any entry type.
///
/// This is used under the hood by `create_entry`, `create_cap_grant` and `create_cap_claim`.
///
/// The host builds a `Create` header for the passed entry value and commits a new element to the
/// chain.
///
/// Usually you don't need to use this macro directly but it is the most general way to create an
/// entry and standardises the internals of higher level create macros.
///
/// @see create_entry
/// @see create_cap_grant
/// @see create_cap_claim
#[macro_export]
macro_rules! create {
    ( $type:expr, $entry:expr ) => {{
        $crate::prelude::host_externs!(__create);
        $crate::host_fn!(
            __create,
            $crate::prelude::CreateInput::new(($type.into(), $entry.into(),)),
            $crate::prelude::CreateOutput
        )
    }};
}
