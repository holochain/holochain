#[macro_export]
macro_rules! zome_info {
    () => {{
        $crate::host_fn!(
            __zome_info,
            $crate::prelude::ZomeInfoInput::new(()),
            $crate::prelude::ZomeInfoOutput
        )
    }};
}
