pub mod sql_cell {
    pub(crate) const SCHEMA: &str = include_str!("sql/cell/schema.sql");
    pub const UPDATE_INTEGRATE_OPS: &str = include_str!("sql/cell/update_integrate_ops.sql");
}

pub(crate) mod sql_conductor {
    pub(crate) const SCHEMA: &str = include_str!("sql/conductor/schema.sql");
}

pub(crate) mod sql_wasm {
    pub(crate) const SCHEMA: &str = include_str!("sql/wasm/schema.sql");
}

pub(crate) mod sql_p2p_state {
    pub(crate) const SCHEMA: &str = include_str!("sql/p2p_state/schema.sql");
    pub(crate) const INSERT: &str = include_str!("sql/p2p_state/insert.sql");
    pub(crate) const SELECT_ALL: &str = include_str!("sql/p2p_state/select_all.sql");
    pub(crate) const SELECT: &str = include_str!("sql/p2p_state/select.sql");
    pub(crate) const GOSSIP_QUERY: &str = include_str!("sql/p2p_state/gossip_query.sql");
    pub(crate) const PRUNE: &str = include_str!("sql/p2p_state/prune.sql");
}

pub(crate) mod sql_p2p_metrics {
    pub(crate) const SCHEMA: &str = include_str!("sql/p2p_metrics/schema.sql");
}
