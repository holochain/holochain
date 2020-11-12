/// General macro that can update any entry type.
///
/// This is used under the hood by `update_entry`, `update_cap_grant!` and `update_cap_claim!`.
/// @todo implement update_cap_claim
///
/// The host builds an `Update` header for the passed entry value and commits a new update to the
/// chain.
///
/// Usually you don't need to use this macro directly but it is the most general way to update an
/// entry and standardises the internals of higher level create macros.
///
/// @see update_entry
/// @see update_cap_grant!
/// @see update_cap_claim!
#[macro_export]
macro_rules! update {
    ( $hash:expr, $type:expr, $input:expr ) => {{
        $crate::prelude::host_externs!(__update);

        $crate::host_fn!(
            __update,
            $crate::prelude::UpdateInput::new(($type, $input, $hash)),
            $crate::prelude::UpdateOutput
        )
    }};
}
