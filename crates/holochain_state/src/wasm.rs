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
        Some((hash, wasm)) => Ok(Some(DnaWasmHashed::with_pre_hashed(wasm.into(), hash))),
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
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_types::dna::wasm::DnaWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_store_round_trip() -> DatabaseResult<()> {
        holochain_trace::test_run();

        // all the stuff needed to have a WasmBuf
        let db = crate::test_utils::test_wasm_db();

        // a wasm
        let wasm =
            DnaWasmHashed::from_content(DnaWasm::from(holochain_wasm_test_utils::TestWasm::Foo))
                .await;

        // Put wasm
        db.write_async({
            let put_wasm = wasm.clone();

            move |txn| put(txn, put_wasm.clone())
        })
        .await
        .unwrap();
        db.read_async(move |txn| -> DatabaseResult<()> {
            assert!(contains(txn, wasm.as_hash()).unwrap());
            // a wasm from the WasmBuf
            let ret = get(txn, wasm.as_hash()).unwrap().unwrap();

            // assert the round trip
            assert_eq!(ret, wasm);

            Ok(())
        })
        .await?;

        Ok(())
    }
}
