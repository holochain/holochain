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

pub(crate) mod sql_p2p {
    pub(crate) const SCHEMA: &str = include_str!("sql/p2p/schema.sql");
    pub(crate) const INSERT: &str = include_str!("sql/p2p/insert.sql");
    pub(crate) const SELECT_ALL: &str = include_str!("sql/p2p/select_all.sql");
    pub(crate) const SELECT: &str = include_str!("sql/p2p/select.sql");
    pub(crate) const GOSSIP_QUERY: &str = include_str!("sql/p2p/gossip_query.sql");
    pub(crate) const QUERY_NEAR_BASIS: &str = include_str!("sql/p2p/query_near_basis.sql");
    pub(crate) const PRUNE: &str = include_str!("sql/p2p/prune.sql");
}
