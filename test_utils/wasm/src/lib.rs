use sx_types::dna::wasm::DnaWasm;

pub enum TestWasm {
    Debug,
    Foo,
    Imports,
}

impl From<TestWasm> for DnaWasm {
    fn from(test_wasm: TestWasm) -> DnaWasm {
        DnaWasm::from(
            std::fs::read(
                format!("{}/wasm32-unknown-unknown/release/test_wasm_{}.wasm",
                env!("OUT_DIR"),
                match test_wasm {
                    TestWasm::Debug => "debug",
                    TestWasm::Foo => "foo",
                    TestWasm::Imports => "imports",
                })
            )
            .unwrap()
        )
    }
}
