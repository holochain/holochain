use holo_hash::WasmHash;
use holochain_state::error::{DatabaseError, DatabaseResult};
use holochain_state::exports::SingleStore;
use holochain_state::{
    buffer::CasBufFreshAsync,
    prelude::{BufferedStore, EnvironmentRead},
    transaction::Writer,
};
use holochain_types::dna::wasm::{DnaWasm, DnaWasmHashed};

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
    use holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(threaded_scheduler)]
    async fn wasm_store_round_trip() -> DatabaseResult<()> {
        use holochain_state::prelude::*;
        holochain_types::observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let env = holochain_state::test_utils::test_wasm_env();
        let dbs = env.dbs();
        let mut wasm_buf = WasmBuf::new(
            env.env().into(),
            dbs.get_db(&*holochain_state::db::WASM).unwrap(),
        )
        .unwrap();

        // a wasm
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
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
