use holochain_types::prelude::DnaWasm;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestWasmPair;

#[test]
fn can_get_code() {
    let _dna: TestWasmPair<DnaWasm> = TestWasm::AgentInfo.into();
}
