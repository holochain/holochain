use std::path::PathBuf;
use sx_types::dna::wasm::DnaWasm;
use std::io::Read;

pub enum TestWasm {
    Foo,
    Imports,
}

impl From<TestWasm> for DnaWasm {
    fn from(test_wasm: TestWasm) -> DnaWasm {
        DnaWasm::from(
            include_bytes!(concat!(
                env!("OUT_DIR"),
                format!("/wasm32-unknown-unknown/release/test_wasm_{}.wasm",
                match test_wasm {
                    TestWasm::Foo => "foo",
                    TestWasm::Imports => "imports",
                })
            ))
            .to_vec(),
        )
    }
}
