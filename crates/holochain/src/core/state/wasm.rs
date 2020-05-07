use holo_hash::WasmHash;
use holochain_state::buffer::CasBuf;
use holochain_state::error::DatabaseResult;
use holochain_state::exports::SingleStore;
use holochain_state::transaction::Readable;
use holochain_state::transaction::Reader;
use holochain_types::dna::wasm::DnaWasm;
use std::convert::TryInto;

pub type WasmCas<'env, R> = CasBuf<'env, DnaWasm, R>;

/// This is where wasm lives
pub struct WasmBuf<'env, R: Readable = Reader<'env>> {
    wasm: WasmCas<'env, R>,
}

impl<'env, R: Readable> WasmBuf<'env, R> {
    // @TODO use this code so it isn't dead
    #[allow(dead_code)]
    fn new(reader: &'env R, wasm_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            wasm: CasBuf::new(reader, wasm_store)?,
        })
    }

    pub fn get(&self, wasm_hash: WasmHash) -> DatabaseResult<Option<DnaWasm>> {
        self.wasm.get(&wasm_hash.into())
    }

    pub fn put(&mut self, v: DnaWasm) -> DatabaseResult<()> {
        self.wasm.put((&v).try_into()?, v);
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
    let wasm = DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo);
    let hash = holo_hash::WasmHash::with_data_sync(&wasm.code());

    // a wasm in the WasmBuf
    wasm_buf.put(wasm.clone()).unwrap();
    // a wasm from the WasmBuf
    let ret = wasm_buf.get(hash).unwrap().unwrap();

    // assert the round trip
    assert_eq!(ret, wasm);

    Ok(())
}
