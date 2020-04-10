use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::SysTimeInput;
use sx_zome_types::SysTimeOutput;

pub fn sys_time(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SysTimeInput,
) -> SysTimeOutput {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    SysTimeOutput::new(since_the_epoch)
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::test_ribosome;
    use crate::core::ribosome::wasm_test::zome_invocation_from_names;
    use crate::core::ribosome::RibosomeT;
    use std::convert::TryFrom;
    use std::convert::TryInto;
    use std::time::Duration;
    use sx_types::nucleus::ZomeInvocationResponse;
    use sx_types::shims::SourceChainCommitBundle;
    use sx_zome_types::zome_io::SysTimeOutput;
    use sx_zome_types::SysTimeInput;

    #[test]
    fn invoke_import_sys_time_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "sys_time",
            SysTimeInput::new(()).try_into().unwrap(),
        );

        let output: Duration = match ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
        {
            Ok(ZomeInvocationResponse::ZomeApiFn(guest_output)) => {
                SysTimeOutput::try_from(guest_output.inner())
                    .unwrap()
                    .inner()
            }
            _ => unreachable!(),
        };

        dbg!(output);

        // this is non-deterministic but may have some use in a "soft fail" kind of way?
        // let test_now = now();
        //
        // // if it takes more than 2 ms to read the system time something is horribly wrong
        // assert!(
        //     (i128::try_from(test_now.as_millis()).unwrap()
        //         - i128::try_from(output.as_millis()).unwrap())
        //     .abs()
        //         < 3
        // );
    }
}
