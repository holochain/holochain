use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::SleepInput;
use sx_zome_types::SleepOutput;

pub fn sleep(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: SleepInput,
) -> SleepOutput {
    std::thread::sleep(input.inner());
    SleepOutput::new(())
}

// #[cfg(all(test, feature = "wasmtest"))]
// pub mod wasm_test {
//     use crate::core::ribosome::wasm_test::now;
//     use crate::core::ribosome::wasm_test::test_ribosome;
//     use crate::core::ribosome::wasm_test::zome_invocation_from_names;
//     use crate::core::ribosome::RibosomeT;
//     use std::convert::TryFrom;
//     use std::convert::TryInto;
//     use std::time::Duration;
//     use sx_types::shims::SourceChainCommitBundle;
//     use sx_zome_types::SleepInput;
//
//     // @TODO
//     // this test is non-deterministic, so we're not using it at the moment
//     // the question is whether we might want to enable it behind a compiler flag
//     // is there value in having assertions representing our "SLAs"?
//     // if
//     // #[test]
//     // fn invoke_import_sleep_test() {
//     //     test_ribosome()
//     //         .call_zome_function(
//     //             &mut SourceChainCommitBundle::default(),
//     //             zome_invocation_from_names(
//     //                 "imports",
//     //                 "sleep",
//     //                 SleepInput::new(Duration::from_millis(0))
//     //                     .try_into()
//     //                     .unwrap(),
//     //             ),
//     //         )
//     //         .unwrap();
//     //
//     //     let ribosome = test_ribosome();
//     //
//     //     let t0 = now().as_millis();
//     //
//     //     let invocation = zome_invocation_from_names(
//     //         "imports",
//     //         "sleep",
//     //         SleepInput::new(Duration::from_millis(0))
//     //             .try_into()
//     //             .unwrap(),
//     //     );
//     //
//     //     ribosome
//     //         .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
//     //         .unwrap();
//     //     let t1 = now().as_millis();
//     //
//     //     let diff0 = i128::try_from(t1).unwrap() - i128::try_from(t0).unwrap();
//     //
//     //     assert!(diff0 < 2, format!("t0, t1, diff0: {} {} {}", t0, t1, diff0));
//     //
//     //     let ribosome = test_ribosome();
//     //
//     //     let t2 = now();
//     //
//     //     let invocation = zome_invocation_from_names(
//     //         "imports",
//     //         "sleep",
//     //         SleepInput::new(Duration::from_millis(3))
//     //             .try_into()
//     //             .unwrap(),
//     //     );
//     //
//     //     ribosome
//     //         .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
//     //         .unwrap();
//     //     let t3 = now();
//     //
//     //     let diff1 =
//     //         i128::try_from(t3.as_millis()).unwrap() - i128::try_from(t2.as_millis()).unwrap();
//     //
//     //     println!("{}", diff1);
//     //     assert!(2 < diff1);
//     //     assert!(diff1 < 5);
//     // }
// }
