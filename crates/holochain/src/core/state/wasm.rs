use holo_hash::{Hashable, Hashed, WasmHash};
use holochain_state::buffer::{BufferedStore, CasBuf};
use holochain_state::error::{DatabaseError, DatabaseResult};
use holochain_state::exports::SingleStore;
use holochain_state::transaction::Readable;
use holochain_state::transaction::{Reader, Writer};
use holochain_types::dna::wasm::{DnaWasm, DnaWasmHashed};

pub type WasmCas<'env> = CasBuf<'env, DnaWasmHashed>;

/// This is where wasm lives
pub struct WasmBuf<'env>(WasmCas<'env>);

impl<'env> WasmBuf<'env> {
    pub fn new(reader: &'env Reader<'env>, wasm_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self(CasBuf::new(reader, wasm_store)?))
    }

    pub async fn get(&self, wasm_hash: &WasmHash) -> DatabaseResult<Option<DnaWasmHashed>> {
        self.0.get(wasm_hash).await
    }

    pub fn put(&mut self, v: DnaWasmHashed) {
        self.0.put(v);
    }
}

impl<'env> BufferedStore<'env> for WasmBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}

#[tokio::test(threaded_scheduler)]
async fn wasm_store_round_trip() -> DatabaseResult<()> {
    use holochain_state::env::ReadManager;
    use holochain_state::prelude::*;
    holochain_types::observability::test_run().ok();

    // all the stuff needed to have a WasmBuf
    let env = holochain_state::test_utils::test_wasm_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let mut wasm_buf =
        WasmBuf::new(&reader, dbs.get_db(&*holochain_state::db::WASM).unwrap()).unwrap();

    // a wasm
    let wasm =
        DnaWasmHashed::with_data(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo)).await?;

    // a wasm in the WasmBuf
    wasm_buf.put(wasm.clone());
    // a wasm from the WasmBuf
    let ret = wasm_buf.get(&wasm.as_hash()).await.unwrap().unwrap();

    // assert the round trip
    assert_eq!(ret, wasm);

    Ok(())
}
