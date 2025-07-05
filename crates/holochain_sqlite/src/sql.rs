pub mod sql_cell {
    pub const SET_VALIDATED_OPS_TO_INTEGRATED: &str =
        include_str!("sql/cell/set_validated_ops_to_integrated.sql");

    pub const ACTIVITY_INTEGRATED_UPPER_BOUND: &str =
        include_str!("sql/cell/activity_integrated_upper_bound.sql");
    pub const ACTION_HASH_BY_PREV: &str = include_str!("sql/cell/action_hash_by_prev.sql");
    pub const ALL_ACTIVITY_AUTHORS: &str = include_str!("sql/cell/all_activity_authors.sql");
    pub const ALL_READY_ACTIVITY: &str = include_str!("sql/cell/all_ready_activity.sql");
    pub const DELETE_ACTIONS_AFTER_SEQ: &str =
        include_str!("sql/cell/delete_actions_after_seq.sql");

    pub const SELECT_VALID_AGENT_PUB_KEY: &str =
        include_str!("sql/cell/select_valid_agent_pub_key.sql");

    pub const FETCH_OP_HASHES_P1: &str =
        include_str!("sql/cell/fetch_hashes/fetch_op_hashes_p1.sql");
    pub const FETCH_OP_HASHES_P2: &str =
        include_str!("sql/cell/fetch_hashes/fetch_op_hashes_p2.sql");

    pub const FETCH_OP_REGION: &str = include_str!("sql/cell/fetch_op_region.sql");
    pub const FETCH_OPS_BY_REGION: &str = include_str!("sql/cell/fetch_ops_by_region.sql");
    pub const FETCH_REGION_OP_HASHES: &str = include_str!("sql/cell/fetch_region_op_hashes.sql");

    pub const FETCH_PUBLISHABLE_OP: &str = include_str!("sql/cell/fetch_publishable_op.sql");

    pub mod must_get_agent_activity {
        pub const MUST_GET_AGENT_ACTIVITY: &str =
            include_str!("sql/cell/agent_activity/must_get_agent_activity.sql");
        pub const ACTION_HASH_TO_SEQ: &str =
            include_str!("sql/cell/agent_activity/action_hash_to_seq.sql");
        pub const ACTION_TS_TO_SEQ: &str =
            include_str!("sql/cell/agent_activity/action_ts_to_seq.sql");
    }

    pub mod schedule {
        pub const UPDATE: &str = include_str!("sql/cell/schedule/update.sql");
        pub const DELETE: &str = include_str!("sql/cell/schedule/delete.sql");
        pub const EXPIRED: &str = include_str!("sql/cell/schedule/expired.sql");
        pub const DELETE_ALL_EPHEMERAL: &str =
            include_str!("sql/cell/schedule/delete_all_ephemeral.sql");
        pub const DELETE_LIVE_EPHEMERAL: &str =
            include_str!("sql/cell/schedule/delete_live_ephemeral.sql");
    }
    pub mod state_dump {
        pub const DHT_OPS_IN_INTEGRATION_LIMBO: &str =
            include_str!("sql/cell/state_dump/dht_ops_in_integration_limbo.sql");
        pub const DHT_OPS_INTEGRATED: &str =
            include_str!("sql/cell/state_dump/dht_ops_integrated.sql");
        pub const DHT_OPS_IN_VALIDATION_LIMBO: &str =
            include_str!("sql/cell/state_dump/dht_ops_in_validation_limbo.sql");
        pub const DHT_OPS_ROW_ID: &str = include_str!("sql/cell/state_dump/dht_ops_row_id.sql");
    }
}

pub mod sql_dht {
    pub const OP_HASHES_IN_TIME_SLICE: &str = include_str!("sql/dht/op_hashes_in_time_slice.sql");

    pub const OP_HASHES_SINCE_TIME_BATCH: &str =
        include_str!("sql/dht/op_hashes_since_time_batch.sql");

    pub const OPS_BY_ID: &str = include_str!("sql/dht/ops_by_id.sql");

    pub const CHECK_OP_IDS_PRESENT: &str = include_str!("sql/dht/check_op_ids_present.sql");

    pub const EARLIEST_TIMESTAMP: &str = include_str!("sql/dht/earliest_timestamp.sql");
}

pub mod sql_conductor {
    pub(crate) const SELECT_NONCE: &str = include_str!("sql/conductor/nonce_already_seen.sql");
    pub const DELETE_EXPIRED_NONCE: &str = include_str!("sql/conductor/delete_expired_nonce.sql");
    pub const FROM_BLOCK_SPAN_WHERE_OVERLAPPING: &str =
        include_str!("sql/conductor/from_block_span_where_overlapping.sql");
    pub const IS_BLOCKED: &str = include_str!("sql/conductor/is_blocked.sql");
    pub const SELECT_VALID_CAP_GRANT_FOR_CAP_SECRET: &str =
        include_str!("sql/conductor/select_valid_cap_grant_for_cap_secret.sql");
    pub const SELECT_VALID_UNRESTRICTED_CAP_GRANT: &str =
        include_str!("sql/conductor/select_valid_unrestricted_cap_grant.sql");
}

pub(crate) mod sql_wasm {}

pub mod sql_peer_meta_store {
    pub const PRUNE: &str = include_str!("sql/peer_meta_store/prune.sql");

    pub const INSERT: &str = include_str!("sql/peer_meta_store/insert.sql");

    pub const GET: &str = include_str!("sql/peer_meta_store/get.sql");

    pub const GET_ALL_BY_KEY: &str = include_str!("sql/peer_meta_store/get_all_by_key.sql");

    pub const DELETE: &str = include_str!("sql/peer_meta_store/delete.sql");

    pub const DELETE_URLS: &str = include_str!("sql/peer_meta_store/delete_urls.sql");

    pub const GET_ALL_BY_URL: &str = include_str!("sql/peer_meta_store/get_all_by_url.sql");
}
