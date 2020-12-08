use holo_hash::WasmHash;
use crate::holochain_state::buffer::CasBufFreshAsync;
use crate::holochain_state::error::DatabaseError;
use crate::holochain_state::error::DatabaseResult;
use crate::holochain_state::exports::SingleStore;
use crate::holochain_state::prelude::BufferedStore;
use crate::holochain_state::prelude::EnvironmentRead;
use crate::holochain_state::transaction::Writer;
use crate::holochain_types::dna::wasm::DnaWasm;
use crate::holochain_types::dna::wasm::DnaWasmHashed;

/// This is where wasm lives
pub struct WasmBuf(CasBufFreshAsync<DnaWasm>);

impl WasmBuf {
    pub fn new(env: EnvironmentRead, wasm_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self(CasBufFreshAsync::new(env, wasm_store)))
    }

    pub async fn get(&self, wasm_hash: &WasmHash) -> DatabaseResult<Option<DnaWasmHashed>> {
        self.0.get(&wasm_hash).await
    }

    pub fn put(&mut self, v: DnaWasmHashed) {
        self.0.put(v);
    }
}

impl BufferedStore for WasmBuf {
    type Error = DatabaseError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::HasHash;
    use crate::holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(threaded_scheduler)]
    async fn wasm_store_round_trip() -> DatabaseResult<()> {
        use crate::holochain_state::prelude::*;
        crate::holochain_types::observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let env = crate::holochain_state::test_utils::test_wasm_env();
        let mut wasm_buf = WasmBuf::new(
            env.env().into(),
            env.get_db(&*crate::holochain_state::db::WASM).unwrap(),
        )
        .unwrap();

        // a wasm
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(crate::holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        // a wasm in the WasmBuf
        wasm_buf.put(wasm.clone());
        // a wasm from the WasmBuf
        let ret = wasm_buf.get(&wasm.as_hash()).await.unwrap().unwrap();

        // assert the round trip
        assert_eq!(ret, wasm);

        Ok(())
    }
}
