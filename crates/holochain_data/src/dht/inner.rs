//! Free-standing `async fn`s over `sqlx::Executor` for each DHT domain.
//!
//! Each submodule covers a single SQL table (or — for `entry` — the
//! `Entry` and `PrivateEntry` pair), with the exception of `sync_queries`
//! which holds cross-table queries needed by the Kitsune2 op-store
//! contract. The handle layers (`db_operations` / `tx_operations`) are
//! thin wrappers that acquire the appropriate executor and delegate here.

pub(crate) mod action;
pub(crate) mod cap_claim;
pub(crate) mod cap_grant;
pub(crate) mod chain_lock;
pub(crate) mod chain_op;
pub(crate) mod chain_op_publish;
pub(crate) mod db_size;
pub(crate) mod deleted_link;
pub(crate) mod deleted_record;
pub(crate) mod entry;
pub(crate) mod limbo_chain_op;
pub(crate) mod limbo_warrant;
pub(crate) mod link;
pub(crate) mod move_to_limbo;
pub(crate) mod op_exists;
pub(crate) mod remove_countersigning_session;
pub(crate) mod scheduled_function;
pub(crate) mod slice_hash;
pub(crate) mod sync_queries;
pub(crate) mod updated_record;
pub(crate) mod validation_receipt;
pub(crate) mod warrant;
pub(crate) mod warrant_publish;
