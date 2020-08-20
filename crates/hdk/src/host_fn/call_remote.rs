#[macro_export]
macro_rules! call_remote {
    ( $agent:expr, $zome:expr, $fn_name:expr, $cap:expr, $request:expr ) => {{
        $crate::host_fn!(
            __call_remote,
            $crate::prelude::CallRemoteInput::new($crate::prelude::CallRemote::new(
                $agent, $zome, $fn_name, $cap, $request
            )),
            $crate::prelude::CallRemoteOutput
        )
    }};
}
