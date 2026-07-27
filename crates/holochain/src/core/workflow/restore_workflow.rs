//! This workflow reconstructs an agent's source chain from the DHT in place of genesis for a cell
//! installed with `restore_from_dht: true`. The workflow itself loops internally, retrying with a
//! backoff, until it reaches a terminal [`RestoreOutcome`].
//!
//! Each attempt of the workflow follows these distinct steps:
//!
//!
//! * **Step 1**, in [`agent_activity`], gets the agent's chain activity from the DHT, aggregates
//!   the responses, requires unanimous agreement on the chain head from the peers that responded,
//!   then collects the verified `Record`s. It also collects any warrants naming the agent
//!   regardless of whether a head was agreed this round.
//! * **Step 2**, in [`warrants`], runs only when responses from Step 1 include warrants against the
//!   agent whose chain is being restored. It stages the received warrants for local validation and
//!   polls for a verdict. If any single warrant is validated then the restore will fail permanently
//!   for this cell. If no warrants were received or all warrants are rejected then the attempt
//!   proceeds to Step 3 using the chain head that was agreed upon in Step 1.
//! * **Step 3**, in [`chain_reconstruction`], walks the collected records backward from the agreed
//!   head to genesis, then writes the verified chain directly into the per-DNA database as authored
//!   state, this bypasses validation limbo.
//!
//! Reporting completion to the per-app orchestrator, emitting system signals, and app status
//! transitions are the responsibility of the orchestrator.

pub(crate) mod agent_activity;
pub(crate) mod chain_reconstruction;
pub(crate) mod warrants;
