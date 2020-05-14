use holo_hash::{Hashed, WasmHash};
use holochain_state::buffer::{BufferedStore, CasBuf};
use holochain_state::error::{DatabaseError, DatabaseResult};
use holochain_state::exports::SingleStore;
use holochain_state::transaction::Readable;
use holochain_state::transaction::{Reader, Writer};
use holochain_types::dna::wasm::{DnaWasm, DnaWasmHashed};

pub type WasmCas<'env, R> = CasBuf<'env, DnaWasm, R>;

/// This is where wasm lives
pub struct WasmBuf<'env, R: Readable = Reader<'env>> {
    wasm: WasmCas<'env, R>,
}

impl<'env, R: Readable> WasmBuf<'env, R> {
    pub fn new(reader: &'env R, wasm_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            wasm: CasBuf::new(reader, wasm_store)?,
        })
    }

    pub async fn get(&self, wasm_hash: &WasmHash) -> DatabaseResult<Option<DnaWasmHashed>> {
        match self.wasm.get(&wasm_hash.clone().into())? {
            None => Ok(None),
            Some(wasm) => {
                let wasm = DnaWasmHashed::with_data(wasm).await?;
                fatal_db_hash_check!("WasmBuf::get", wasm_hash, wasm.as_hash());
                Ok(Some(wasm))
            }
        }
    }

    pub async fn put(&mut self, v: DnaWasm) -> DatabaseResult<WasmHash> {
        let v = DnaWasmHashed::with_data(v).await?;
        let (wasm, wasm_hash) = v.into_inner();
        self.wasm.put(wasm_hash.clone().into(), wasm);
        Ok(wasm_hash)
    }
}

impl<'env, R> BufferedStore<'env> for WasmBuf<'env, R>
where
    R: Readable,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.wasm.flush_to_txn(writer)?;
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

    // a wasm in the WasmBuf
    let hash = wasm_buf.put(wasm.clone()).await.unwrap();
    // a wasm from the WasmBuf
    let ret = wasm_buf.get(&hash).await.unwrap().unwrap();

    // assert the round trip
    assert_eq!(ret, DnaWasmHashed::with_data(wasm).await.unwrap());

    Ok(())
}
