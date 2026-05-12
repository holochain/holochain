//! `DbRead<Dht>` / `DbWrite<Dht>` method API, split per DHT domain.
//!
//! Each submodule adds methods to the `DbRead<Dht>` / `DbWrite<Dht>` handle
//! types via `impl` blocks. The methods delegate to free fns in
//! [`super::inner`].

mod action;
mod cap_claim;
mod cap_grant;
mod chain_lock;
mod chain_op;
mod chain_op_publish;
mod deleted_link;
mod deleted_record;
mod entry;
mod limbo_chain_op;
mod limbo_warrant;
mod link;
mod scheduled_function;
mod updated_record;
mod validation_receipt;
mod warrant;
mod warrant_publish;
