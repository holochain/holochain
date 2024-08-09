//! Holochain application migration guide
//!
//! # Agent key update (key migration)
//!
//! Holochain conductors with the DPKI service installed enable agents to update their keys.
//! An agent key is updated by migrating all apps belonging to the agent from the old agent key to
//! a new agent key. This guide describes the steps to execute this agent key update and outlines
//! the migration process of one application as an example.
//!
//! Here is an overview of this migration process:
//! - All source chains of the cells that the app contains are closed.
//! - A new key pair in DPKI is generated for the agent.
//! - The new app with new cells is created and cryptographically linked to the old cells.
//!
//! What follows is the breakdown of the high level steps.
//!
//! ## Close source chains
//! The special action `Close` is written to each of the cells' source chains of the app to be migrated.
//! It is the final action of the chains and no further actions can be added to them. The hash of the
//! closing action will be passed to the opening action of the new source chain as a forward link between
//! the old and the new cell.
//!
//! ## Generate new key pair in DPKI
//! Agents can only update their key with DPKI installed as a conductor service. A new key is derived
//! for the agent and registered in DPKI. Automatically the old key is invalidated and no longer has
//! any permission to modify the agent's source chain.
//!
//! In case the app uses deferred membrane proofs, the remaining steps are similar to when installing
//! an app. App information of a new app with the new agent key and new cell ids is returned to the
//! requesting client, and membrane proofs are awaited to be provisioned. While awaiting the proofs,
//! the app is disabled and no other interaction is possible.
//!
//! ## New app creation
//! Based on the same DNA as the old app and the new agent key, new cells are created and added to a new app.
//! The app id of the new app needs to be provided by the client that is requesting the key update.
//! After creating the new cells, an `Open` action is written to them. It contains the action hash of the
//! old chains' `Close` action.
//! Once app creation has completed successfully, the update call returns the app information.
//!
//! Should any of the above steps fail, the key update can be repeated until the new app is successfully
//! created.
