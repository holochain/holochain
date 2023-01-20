//! The API interface into the holochain conductor.
//!
//! The interface is split into admin and app requests and responses. Each has
//! an associated enum [ `AdminRequest` ] and [ `AppRequest` ] that define and
//! document available methods.
//!
//! The admin interface generally manages the conductor itself, such as installing apps, changing coordinators, listing
//! dnas, cells and apps, accessing state and metric information dumps, managing
//! agents, etc.
//!
//! The app interface is smaller and focussed on interfacing with an app directly.
//! Notably the app interface allows calling zome functions and subscribing to
//! signals from the app.

mod admin_interface;
mod app_interface;
pub mod config;
pub mod signal_subscription;
pub mod state_dump;

pub use admin_interface::*;
pub use app_interface::*;
pub use config::*;
pub use state_dump::*;
