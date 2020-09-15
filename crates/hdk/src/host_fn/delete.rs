/// General macro that can delete any entry type.
///
/// This is used under the hood by `delete_entry!`, `delete_cap_grant!` and `delete_cap_claim!`.
/// @todo implement delete_cap_claim
///
/// The host builds a `Delete` header for the passed entry and commits a new element to the chain.
///
/// Usually you don't need to use this macro directly but it is the most general way to update an
/// entry and standardises the internals of higher level create macros.
#[macro_export]
macro_rules! delete {
    ( $hash:expr ) => {{
        $crate::prelude::host_externs!(__delete);

        $crate::host_fn!(
            __delete,
            $crate::prelude::DeleteInput::new($hash.into()),
            $crate::prelude::DeleteOutput
        )
    }};
}
