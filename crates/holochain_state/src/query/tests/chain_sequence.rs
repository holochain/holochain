use crate::{source_chain::SourceChainResult, test_utils::test_cell_db};
use holo_hash::ActionHash;
use holochain_sqlite::prelude::*;
use matches::assert_matches;
use observability;

#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let arc = test_db.env();
    {
        let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
        assert_eq!(buf.chain_head(), None);
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .into(),
        )?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ])
                .into()
            )
        );
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 1,
            ])
            .into(),
        )?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                ])
                .into()
            )
        );
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 2,
            ])
            .into(),
        )?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                ])
                .into()
            )
        );
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_functionality() -> SourceChainResult<()> {
    let test_db = test_cell_db();
    let arc = test_db.env();

    {
        let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 1,
            ])
            .into(),
        )?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                ])
                .into()
            )
        );
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 2,
            ])
            .into(),
        )?;
        arc.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }
    let mut g = arc.conn().unwrap();
    g.with_reader(|mut reader| {
        let buf = ChainSequenceBuf::new(arc.clone().into())?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                ])
                .into()
            )
        );
        let items: Vec<u32> = buf
            .buf
            .store()
            .iter(&mut reader)?
            .map(|(key, _)| Ok(IntKey::from_key_bytes_or_friendly_panic(&key).into()))
            .collect()?;
        assert_eq!(items, vec![0, 1, 2]);
        DatabaseResult::Ok(())
    })?;

    {
        let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 3,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 4,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 5,
            ])
            .into(),
        )?;
        arc.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }
    let mut g = arc.conn().unwrap();
    g.with_reader(|mut reader| {
        let buf = ChainSequenceBuf::new(arc.clone().into())?;
        assert_eq!(
            buf.chain_head(),
            Some(
                &ActionHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5
                ])
                .into()
            )
        );
        let items: Vec<u32> = buf
            .buf
            .store()
            .iter(&mut reader)?
            .map(|(_, i)| Ok(i.tx_seq))
            .collect()?;
        assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
        Ok(())
    })
}

/// If we attempt to move the chain head, but it has already moved from
/// under us, error
#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_head_moved_triggers_error() -> anyhow::Result<()> {
    let test_db = test_cell_db();
    let arc1 = test_db.env();
    let arc2 = test_db.env();
    let (tx1, rx1) = tokio::sync::oneshot::channel();
    let (tx2, rx2) = tokio::sync::oneshot::channel();

    // Attempt to move the chain concurrently-- this one fails
    let task1 = tokio::spawn(async move {
        let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 1,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 2,
            ])
            .into(),
        )?;

        // let the other task run and make a commit to the chain head,
        // which will cause this one to error out when it re-enters and tries to commit
        tx1.send(()).unwrap();
        rx2.await.unwrap();

        arc1.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    });

    // Attempt to move the chain concurrently -- this one succeeds
    let task2 = tokio::spawn(async move {
        rx1.await.unwrap();
        let mut buf = ChainSequenceBuf::new(arc2.clone().into())?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 3,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 4,
            ])
            .into(),
        )?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 5,
            ])
            .into(),
        )?;

        arc2.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        tx2.send(()).unwrap();
        Result::<_, SourceChainError>::Ok(())
    });

    let (result1, result2) = tokio::join!(task1, task2);

    let expected_hash = ActionHash::from_raw_36(vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 5,
    ])
    .into();
    assert_matches!(
        result1.unwrap(),
        Err(SourceChainError::HeadMoved(
            None,
            Some(
                hash
            )
        ))
        if hash == expected_hash
    );
    assert!(result2.unwrap().is_ok());

    Ok(())
}

/// If the chain head has moved from under us, but we are not moving the
/// chain head ourselves, proceed as usual
#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_head_moved_triggers_no_error_if_clean() -> anyhow::Result<()> {
    let test_db = test_cell_db();
    let arc1 = test_db.env();
    let arc2 = test_db.env();
    let (tx1, rx1) = tokio::sync::oneshot::channel();
    let (tx2, rx2) = tokio::sync::oneshot::channel();

    // Add a few things to start with
    let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
    buf.put_action(
        ActionHash::from_raw_36(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0,
        ])
        .into(),
    )?;
    buf.put_action(
        ActionHash::from_raw_36(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 1,
        ])
        .into(),
    )?;
    arc1.conn()
        .unwrap()
        .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;

    // Modify the chain without adding an action -- this succeeds
    let task1 = tokio::spawn(async move {
        let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
        buf.complete_dht_op(0)?;

        // let the other task run and make a commit to the chain head,
        // to demonstrate the chain moving underneath us
        tx1.send(()).unwrap();
        rx2.await.unwrap();

        arc1.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    });

    // Add an action to the chain -- there is no collision, so this succeeds
    let task2 = tokio::spawn(async move {
        rx1.await.unwrap();
        let mut buf = ChainSequenceBuf::new(arc2.clone().into())?;
        buf.put_action(
            ActionHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 2,
            ])
            .into(),
        )?;

        arc2.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        tx2.send(()).unwrap();
        Result::<_, SourceChainError>::Ok(())
    });

    let (result1, result2) = tokio::join!(task1, task2);

    assert!(result1.unwrap().is_ok());
    assert!(result2.unwrap().is_ok());

    Ok(())
}
