use holochain_state::prelude::*;

pub struct OpInfo {
    validation_status: Option<ValidationStatus>,
    when_integrated: Option<Timestamp>,
}

pub fn op_info(txn: &mut Transaction) -> anyhow::Result<Option<OpInfo>> {
    let sql = "
    SELECT 
        DhtOp.validation_status,
        DhtOp.when_integrated
    FROM DhtOp
    JOIN Action On DhtOp.action_hash = Action.hash
    WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
    AND DhtOp.basis_hash = :action_hash
    AND DhtOp.when_integrated IS NOT NULL
    AND DhtOp.validation_status IS NOT NULL
    ";
    Ok(txn
        .prepare(sql)?
        .query_row((), |row: &Row| {
            // let action =
            //     from_blob::<SignedAction>(row.get(row.as_ref().column_index("action_blob")?)?)?;
            // let op_type = row.get(row.as_ref().column_index("dht_type")?)?;
            let validation_status =
                row.get(row.as_ref().column_index("DhtOp.validation_status")?)?;
            let when_integrated = row.get(row.as_ref().column_index("DhtOp.when_integrated")?)?;
            Ok(OpInfo {
                validation_status,
                when_integrated,
            })
        })
        .optional()?)
}
