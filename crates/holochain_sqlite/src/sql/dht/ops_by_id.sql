SELECT
    DhtOp.hash,
    DhtOp.type,
    Action.blob AS action_blob,
    Action.author AS author,
    Entry.blob AS entry_blob
FROM
    DhtOp
        JOIN Action ON DhtOp.action_hash = Action.hash
        LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
    DhtOp.hash in rarray(:hashes)
    AND DhtOp.when_integrated IS NOT NULL
    AND DhtOp.withhold_publish IS NULL
