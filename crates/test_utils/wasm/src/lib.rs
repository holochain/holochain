use holochain_types::dna::wasm::DnaWasm;
pub extern crate strum;
#[macro_use]
extern crate strum_macros;
use holochain_zome_types::zome::ZomeName;

#[derive(EnumIter, Clone, Copy)]
pub enum TestWasm {
    Debug,
    Foo,
    Imports,
    Validate,
    InitPass,
    InitFail,
}

impl From<TestWasm> for ZomeName {
    fn from(test_wasm: TestWasm) -> ZomeName {
        ZomeName::from(match test_wasm {
            TestWasm::Debug => "debug",
            TestWasm::Foo => "foo",
            TestWasm::Imports => "imports",
            TestWasm::Validate => "validate",
            TestWasm::InitPass => "init_pass",
            TestWasm::InitFail => "init_fail",
        })
    }
}

impl From<TestWasm> for DnaWasm {
    fn from(test_wasm: TestWasm) -> DnaWasm {
        DnaWasm::from(match test_wasm {
            TestWasm::Debug => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_debug.wasm"
            ))
            .to_vec(),
            TestWasm::Foo => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_foo.wasm"
            ))
            .to_vec(),
            TestWasm::Imports => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_imports.wasm"
            ))
            .to_vec(),
            TestWasm::Validate => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_validate.wasm"
            ))
            .to_vec(),
            TestWasm::InitPass => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_init_pass.wasm"
            ))
            .to_vec(),
            TestWasm::InitFail => include_bytes!(concat!(
                env!("OUT_DIR"),
                "/wasm32-unknown-unknown/release/test_wasm_init_fail.wasm"
            ))
            .to_vec(),
        })
    }
}
