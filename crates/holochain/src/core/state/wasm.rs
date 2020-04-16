use sx_types::dna::wasm::DnaWasm;
use sx_state::error::DatabaseResult;
use sx_types::persistence::cas::content::Address;
use sx_state::exports::SingleStore;
use sx_state::transaction::Readable;
use sx_state::buffer::CasBuf;
use sx_state::transaction::Reader;

pub type WasmCas<'env, R> = CasBuf<'env, DnaWasm, R>;

/// This is where wasm lives
pub struct WasmBuf<'env, R: Readable = Reader<'env>> {
    wasm: WasmCas<'env, R>,
}

impl<'env, R: Readable> WasmBuf<'env, R> {
    fn new(
        reader: &'env R,
        wasm_store: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            wasm: CasBuf::new(reader, wasm_store)?,
        })
    }

    pub fn get(&self, dna_hash: DnaHash) -> DatabaseResult<Option<DnaWasm>> {
        self.wasm_store.get(Address::new(format!("{}", dna_hash)))
    }

    pub fn put(&mut self, v: DnaWasm) {
        self.wasm_store.put(v);
    }
}

#[tokio::test]
async fn wasm_return() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_wasm_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let wasm_buf = WasmBuf::new(reader, dbs.get(&*WASM).unwrap());

    let wasm = DnaWasm::from(TestWasm::Foo);
    let hash: DnaHash = DnaHash::with_data_sync(wasm.code.code());

    wasm_buf.put(wasm);
    wasm_buf.get(address);

    Ok(())

    // let Chains {
    //     mut source_chain,
    //     cache,
    //     jimbo_id,
    //     jimbo,
    //     jessy_id,
    //     jessy,
    //     mut mock_primary_meta,
    //     mut mock_cache_meta,
    // } = setup_env(&reader, &dbs)?;
    // source_chain.put_entry(jimbo.clone(), &jimbo_id);
    // source_chain.put_entry(jessy.clone(), &jessy_id);
    // let base = jimbo.address();
    // let target = jessy.address();
    // let result = target.clone();
    //
    // // Return empty links
    // mock_primary_meta
    //     .expect_get_links()
    //     .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
    //     .returning(move |_, _| Ok(HashSet::new()));
    // // Return a link between entries
    // mock_cache_meta
    //     .expect_get_links()
    //     .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
    //     .returning(move |_, _| Ok(hashset! {target.clone()}));
    //
    // // call dht_get_links with above base
    // let cascade = Cascade::new(
    //     &source_chain.cas(),
    //     &mock_primary_meta,
    //     &cache.cas(),
    //     &mock_cache_meta,
    // );
    // let links = cascade.dht_get_links(base, "").await?;
    // // check it returns
    // assert_eq!(links, hashset! {result});
    // Ok(())
}
