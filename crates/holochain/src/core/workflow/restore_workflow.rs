//! This workflow reconstructs an agent's source chain from the DHT in place of genesis for a cell
//! installed with `restore_from_dht: true`.
//!
//! The workflow follows these distinct steps:
//!
//! * **Step 1**, in [`agent_activity`], gets the agent activity from the DHT, aggregates the
//!   responses, checks for agreement on the chain head from the peers that responded, then collects
//!   the verified `Record`s.
//! * **Step 2**, in [`warrants`], runs only when responses from Step 1 include warrants against the
//!   agent whose chain is being restored. It submits the received warrants for local validation
//!   and, if all warrants are rejected, proceeds to Step 3. Otherwise, if any single warrant is
//!   validated, restore will fail permanently for this cell.
//! * **Step 3**, in [`chain_reconstruction`], walks the collected records backward from the agreed
//!   head, then writes the verified chain directly into the per-DNA database as authored state,
//!   this bypasses validation limbo.
//! * **Step 4** reports completion to the per-app orchestrator and emits a restore complete system
//!   signal with the cell_id.

pub(crate) mod agent_activity;
pub(crate) mod chain_reconstruction;
pub(crate) mod warrants;
