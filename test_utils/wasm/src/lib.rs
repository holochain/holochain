use std::path::PathBuf;
use sx_types::dna::wasm::DnaWasm;
use std::io::Read;

/// Load WASM from filesystem
pub fn create_wasm_from_file(path: &PathBuf) -> DnaWasm {
    let mut file = std::fs::File::open(path)
        .unwrap_or_else(|err| panic!("Couldn't create WASM from file: {:?}; {}", std::env::current_dir().unwrap().join(path), err));
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    DnaWasm::from(buf)
}

pub enum TestWasm {
    Foo,
}

impl From<TestWasm> for PathBuf {
    fn from(test_wasm: TestWasm) -> PathBuf {
        match test_wasm {
            TestWasm::Foo => {
                "test_utils/wasm/foo/target/wasm32-unknown-unknown/release/test_wasm_foo.wasm"
            }
        }
        .into()
    }
}

pub fn test_wasm(wasm: TestWasm) -> DnaWasm {
    match wasm {
        TestWasm::Foo => DnaWasm::from(
            include_bytes!("../foo/target/wasm32-unknown-unknown/release/test_wasm_foo.wasm")
                .to_vec(),
        ),
    }
}
