use holo_hash::DnaHash;
use holochain_sqlite::db::{DbKindPeerMetaStore, DbWrite, ReadAccess};
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::sql::sql_peer_meta_store::{DELETE, GET, INSERT, PRUNE};
use rusqlite::named_params;
use std::sync::Arc;

#[tokio::test]
async fn insert_read_delete() {
    let peer_meta_store = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(
        DnaHash::from_raw_36(vec![0xdb; 36]),
    )))
    .unwrap();

    let peer_url = kitsune2_api::Url::from_str("ws://test:80/1").unwrap();
    peer_meta_store
        .write_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<()> {
                txn.execute(
                    INSERT,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                        ":meta_value": "test-value".as_bytes(),
                        ":expires_at": None::<u32>,
                    },
                )?;

                Ok(())
            }
        })
        .await
        .unwrap();

    let value = peer_meta_store
        .read_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<Vec<u8>> {
                let value = txn.query_row(
                    GET,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                    },
                    |row| row.get(0),
                )?;

                Ok(value)
            }
        })
        .await
        .unwrap();

    assert_eq!("test-value".as_bytes(), value);

    peer_meta_store
        .write_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<()> {
                txn.execute(
                    DELETE,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                    },
                )?;

                Ok(())
            }
        })
        .await
        .unwrap();

    let row_count = peer_meta_store
        .read_async(move |txn| -> DatabaseResult<u32> {
            let row_count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                row.get::<_, u32>(0)
            })?;

            Ok(row_count)
        })
        .await
        .unwrap();

    assert_eq!(0, row_count);
}

#[tokio::test]
async fn prune() {
    let peer_meta_store = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(
        DnaHash::from_raw_36(vec![0xdb; 36]),
    )))
    .unwrap();

    let peer_url = kitsune2_api::Url::from_str("ws://test:80/1").unwrap();
    peer_meta_store
        .write_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<()> {
                // Insert an expired value
                txn.execute(
                    INSERT,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test-1",
                        ":meta_value": "test-value-1".as_bytes(),
                        ":expires_at": Some(100),
                    },
                )?;

                // and a valid value
                txn.execute(
                    INSERT,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test-2",
                        ":meta_value": "test-value-2".as_bytes(),
                        ":expires_at": Some(kitsune2_api::Timestamp::now().as_micros()),
                    },
                )?;

                Ok(())
            }
        })
        .await
        .unwrap();

    let row_count = peer_meta_store
        .read_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<u32> {
                // Should not be able to get the first value
                txn.query_row(
                    GET,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test-1",
                    },
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .unwrap_err();

                let row_count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                    row.get::<_, u32>(0)
                })?;

                Ok(row_count)
            }
        })
        .await
        .unwrap();

    assert_eq!(2, row_count);

    peer_meta_store
        .write_async(move |txn| -> DatabaseResult<()> {
            txn.execute(PRUNE, [])?;

            Ok(())
        })
        .await
        .unwrap();

    let row_count = peer_meta_store
        .read_async(move |txn| -> DatabaseResult<u32> {
            // Should still be able to get the second value
            txn.query_row(
                GET,
                named_params! {
                    ":peer_url": peer_url.as_str(),
                    ":meta_key": "test-2",
                },
                |row| row.get::<_, Vec<u8>>(0),
            )
            .unwrap();

            let row_count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                row.get::<_, u32>(0)
            })?;

            Ok(row_count)
        })
        .await
        .unwrap();

    assert_eq!(1, row_count);
}

#[tokio::test]
async fn insert_overwrite() {
    let peer_meta_store = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(
        DnaHash::from_raw_36(vec![0xdb; 36]),
    )))
    .unwrap();

    let peer_url = kitsune2_api::Url::from_str("ws://test:80/1").unwrap();
    peer_meta_store
        .write_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<()> {
                txn.execute(
                    INSERT,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                        ":meta_value": "test-value-1".as_bytes(),
                        ":expires_at": None::<u32>,
                    },
                )?;

                // Insert the same key again with a new value
                txn.execute(
                    INSERT,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                        ":meta_value": "test-value-2".as_bytes(),
                        ":expires_at": None::<u32>,
                    },
                )?;

                Ok(())
            }
        })
        .await
        .unwrap();

    let (value, row_count) = peer_meta_store
        .read_async({
            let peer_url = peer_url.clone();
            move |txn| -> DatabaseResult<(Vec<u8>, u32)> {
                let value = txn.query_row(
                    GET,
                    named_params! {
                        ":peer_url": peer_url.as_str(),
                        ":meta_key": "test",
                    },
                    |row| row.get(0),
                )?;

                let row_count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                    row.get::<_, u32>(0)
                })?;

                Ok((value, row_count))
            }
        })
        .await
        .unwrap();

    assert_eq!("test-value-2".as_bytes(), value);
    assert_eq!(1, row_count);
}
