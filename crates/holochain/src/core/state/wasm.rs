use holo_hash::WasmHash;
use std::convert::TryInto;
use sx_state::buffer::CasBuf;
use sx_state::error::DatabaseResult;
use sx_state::exports::SingleStore;
use sx_state::transaction::Readable;
use sx_state::transaction::Reader;
use sx_types::dna::wasm::DnaWasm;

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

#[tokio::test]
async fn wasm_store_round_trip() -> DatabaseResult<()> {
    use sx_state::env::ReadManager;
    sx_types::observability::test_run().ok();

    // all the stuff needed to have a WasmBuf
    let env = sx_state::test_utils::test_wasm_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let mut wasm_buf = WasmBuf::new(&reader, *dbs.get(&*sx_state::db::WASM).unwrap()).unwrap();

    // a wasm
    let wasm = DnaWasm::from(sx_wasm_test_utils::TestWasm::Foo);
    let hash = holo_hash::WasmHash::with_data_sync(&wasm.code());

    // a wasm in the WasmBuf
    wasm_buf.put(wasm.clone()).unwrap();
    // a wasm from the WasmBuf
    let ret = wasm_buf.get(hash).unwrap().unwrap();

    // assert the round trip
    assert_eq!(ret, wasm);

    Ok(())
}
