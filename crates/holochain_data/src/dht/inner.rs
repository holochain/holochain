//! Free-standing `async fn`s over `sqlx::Executor` for each DHT domain.
//!
//! Each submodule mirrors a single SQL table (or — for `entry` — the
//! `Entry` and `PrivateEntry` pair). The handle layers (`db_operations` /
//! `tx_operations`) are thin wrappers that acquire the appropriate executor
//! and delegate here.

pub(crate) mod action;
pub(crate) mod cap_claim;
pub(crate) mod cap_grant;
pub(crate) mod chain_lock;
pub(crate) mod chain_op;
pub(crate) mod chain_op_publish;
pub(crate) mod deleted_link;
pub(crate) mod deleted_record;
pub(crate) mod entry;
pub(crate) mod limbo_chain_op;
pub(crate) mod limbo_warrant;
pub(crate) mod link;
pub(crate) mod scheduled_function;
pub(crate) mod updated_record;
pub(crate) mod validation_receipt;
pub(crate) mod warrant;
pub(crate) mod warrant_publish;
