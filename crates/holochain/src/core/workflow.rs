//! Workflows are the core building block of Holochain functionality.
//!
//! ## Properties
//!
//! Workflows are **transactional**, so that if any workflow fails to run to
//! completion, nothing will happen.
//!
//! In order to achieve this, workflow functions are **free of any side-effects
//! which modify cryptographic state**: they will not modify the source chain
//! nor send network messages which could cause other agents to update their own
//! source chain.
//!
//! Workflows are **never nested**. A workflow cannot call another workflow.
//! However, a workflow can specify that any number of other workflows should
//! be triggered after this one completes.
//!
//! Side effects and triggering of other workflows is specified declaratively
//! rather than imperatively. Each workflow returns a `WorkflowEffects` value
//! representing the side effects that should be run. The `finish` function
//! processes this value and performs the necessary actions, including
//! committing changes to the associated Workspace and triggering other
//! workflows.

pub mod error;

pub mod app_validation_workflow;
pub mod call_zome_workflow;
pub mod genesis_workflow;
pub mod incoming_dht_ops_workflow;
pub mod initialize_zomes_workflow;
pub mod integrate_dht_ops_workflow;
pub mod publish_dht_ops_workflow;
pub mod sys_validation_workflow;
pub mod validation_receipt_workflow;

// TODO: either remove wildcards or add wildcards for all above child modules
pub use call_zome_workflow::*;
pub use genesis_workflow::*;
pub use initialize_zomes_workflow::*;
