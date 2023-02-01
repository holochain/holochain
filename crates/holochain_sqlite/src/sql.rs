pub mod sql_cell {
    pub const UPDATE_INTEGRATE_DEP_ACTIVITY: &str =
        include_str!("sql/cell/update_dep_activity.sql");
    pub const ACTIVITY_INTEGRATED_UPPER_BOUND: &str =
        include_str!("sql/cell/activity_integrated_upper_bound.sql");
    pub const ALL_ACTIVITY_AUTHORS: &str = include_str!("sql/cell/all_activity_authors.sql");
    pub const ALL_READY_ACTIVITY: &str = include_str!("sql/cell/all_ready_activity.sql");
    pub const DELETE_ACTIONS_AFTER_SEQ: &str =
        include_str!("sql/cell/delete_actions_after_seq.sql");
    pub const UPDATE_INTEGRATE_DEP_STORE_RECORD: &str =
        include_str!("sql/cell/update_dep_store_record.sql");
    pub const UPDATE_INTEGRATE_DEP_STORE_ENTRY: &str =
        include_str!("sql/cell/update_dep_store_entry.sql");
    pub const UPDATE_INTEGRATE_DEP_STORE_ENTRY_BASIS: &str =
        include_str!("sql/cell/update_dep_store_entry_basis.sql");
    pub const UPDATE_INTEGRATE_DEP_CREATE_LINK: &str =
        include_str!("sql/cell/update_dep_create_link.sql");

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

pub mod sql_conductor {
    pub(crate) const SELECT_NONCE: &str = include_str!("sql/conductor/nonce_already_seen.sql");
    pub const DELETE_EXPIRED_NONCE: &str = include_str!("sql/conductor/delete_expired_nonce.sql");
}

pub(crate) mod sql_p2p_agent_store {
    pub(crate) const INSERT: &str = include_str!("sql/p2p_agent_store/insert.sql");
    pub(crate) const SELECT_ALL: &str = include_str!("sql/p2p_agent_store/select_all.sql");
    pub(crate) const SELECT: &str = include_str!("sql/p2p_agent_store/select.sql");
    pub(crate) const DELETE: &str = include_str!("sql/p2p_agent_store/delete.sql");
    pub(crate) const GOSSIP_QUERY: &str = include_str!("sql/p2p_agent_store/gossip_query.sql");
    pub(crate) const QUERY_NEAR_BASIS: &str =
        include_str!("sql/p2p_agent_store/query_near_basis.sql");
    pub(crate) const EXTRAPOLATED_COVERAGE: &str =
        include_str!("sql/p2p_agent_store/extrapolated_coverage.sql");
    pub(crate) const PRUNE: &str = include_str!("sql/p2p_agent_store/prune.sql");
}

pub(crate) mod sql_p2p_metrics {
    pub(crate) const INSERT: &str = include_str!("sql/p2p_metrics/insert.sql");
    pub(crate) const PRUNE: &str = include_str!("sql/p2p_metrics/prune.sql");
}

pub(crate) mod sql_wasm {}
