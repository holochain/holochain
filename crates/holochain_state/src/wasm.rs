use std::sync::Arc;

use holo_hash::WasmHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::*;

use crate::mutations;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;

pub fn get(txn: &Transaction<'_>, hash: &WasmHash) -> StateQueryResult<Option<DnaWasmHashed>> {
    let item = txn
        .query_row(
            "SELECT hash, blob FROM Wasm WHERE hash = :hash",
            named_params! {
                ":hash": hash
            },
            |row| {
                let hash: WasmHash = row.get("hash")?;
                let wasm: Vec<u8> = row.get("blob")?;
                Ok((hash, wasm))
            },
        )
        .optional()?;
    match item {
        Some((hash, wasm)) => Ok(Some(DnaWasmHashed::with_pre_hashed(
            DnaWasm {
                code: Arc::new(wasm.into_boxed_slice()),
            },
            hash,
        ))),
        None => Ok(None),
    }
}

pub fn contains(txn: &Transaction<'_>, hash: &WasmHash) -> StateQueryResult<bool> {
    Ok(txn.query_row(
        "SELECT EXISTS(SELECT 1 FROM Wasm WHERE hash = :hash)",
        named_params! {
            ":hash": hash
        },
        |row| row.get(0),
    )?)
}

pub fn put(txn: &mut Transaction, wasm: DnaWasmHashed) -> StateMutationResult<()> {
    mutations::insert_wasm(txn, wasm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::HasHash;
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_store_round_trip() -> DatabaseResult<()> {
        use holochain_sqlite::prelude::*;
        observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let env = crate::test_utils::test_wasm_db();

        // a wasm
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        // Put wasm
        env.conn()?
            .with_commit_sync(|txn| put(txn, wasm.clone()))
            .unwrap();
        fresh_reader_test!(env, |txn| {
            assert!(contains(&txn, &wasm.as_hash()).unwrap());
            // a wasm from the WasmBuf
            let ret = get(&txn, &wasm.as_hash()).unwrap().unwrap();

            // assert the round trip
            assert_eq!(ret, wasm);
        });

        Ok(())
    }
}
