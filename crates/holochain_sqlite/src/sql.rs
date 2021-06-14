pub mod sql_cell {
    pub(crate) const SCHEMA: &str = include_str!("sql/cell/schema.sql");
    pub const UPDATE_INTEGRATE_OPS: &str = include_str!("sql/cell/update_integrate_ops.sql");
    pub const FETCH_OP_HASHES_FULL: &str = include_str!("sql/cell/fetch_op_hashes_full.sql");
    pub const FETCH_OP_HASHES_SINGLE: &str = include_str!("sql/cell/fetch_op_hashes_single.sql");
    pub const FETCH_OP_HASHES_WRAP: &str = include_str!("sql/cell/fetch_op_hashes_wrap.sql");
}

pub(crate) mod sql_conductor {
    pub(crate) const SCHEMA: &str = include_str!("sql/conductor/schema.sql");
}

pub(crate) mod sql_wasm {
    pub(crate) const SCHEMA: &str = include_str!("sql/wasm/schema.sql");
}

pub(crate) mod sql_p2p_agent_store {
    pub(crate) const SCHEMA: &str = include_str!("sql/p2p_agent_store/schema.sql");
    pub(crate) const INSERT: &str = include_str!("sql/p2p_agent_store/insert.sql");
    pub(crate) const SELECT_ALL: &str = include_str!("sql/p2p_agent_store/select_all.sql");
    pub(crate) const SELECT: &str = include_str!("sql/p2p_agent_store/select.sql");
    pub(crate) const GOSSIP_QUERY: &str = include_str!("sql/p2p_agent_store/gossip_query.sql");
    pub(crate) const QUERY_NEAR_BASIS: &str =
        include_str!("sql/p2p_agent_store/query_near_basis.sql");
    pub(crate) const PRUNE: &str = include_str!("sql/p2p_agent_store/prune.sql");
}

pub(crate) mod sql_p2p_metrics {
    pub(crate) const SCHEMA: &str = include_str!("sql/p2p_metrics/schema.sql");
    pub(crate) const INSERT: &str = include_str!("sql/p2p_metrics/insert.sql");
    pub(crate) const QUERY_LAST_SYNC: &str = include_str!("sql/p2p_metrics/query_last_sync.sql");
    pub(crate) const QUERY_OLDEST: &str = include_str!("sql/p2p_metrics/query_oldest.sql");
}
