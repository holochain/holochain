//! Interfaces to manage Holochain applications (hApps) and call their functions.
//!
//! The Conductor is the central component of Holochain. It exposes web sockets for clients to
//! connect to, processes incoming requests and orchestrates data flow and persistence.
//!
//! Refer to [Holochain's architecture](https://developer.holochain.org/concepts/2_application_architecture)
//! for more info. Read about hApp development in the
//! [Happ Development Kit (HDK) documentation](https://docs.rs/hdk/latest/hdk).
//! 
//! There is a [Holochain client for JavaScript](https://github.com/holochain/holochain-client-js)
//! and a [Rust client](https://github.com/holochain/holochain-client-rust)
//! to connect to the Conductor.
//!
//! The Conductor API is split into Admin and App requests and responses. Each has
//! an associated enum [`AdminRequest`] and [`AppRequest`] that define and
//! document available calls.
//!
//! The admin interface generally manages the conductor itself, such as installing apps,
//! listing dnas, cells and apps, accessing state and metric information dumps and managing
//! agents.
//!
//! The app interface is smaller and focussed on interfacing with an app directly.
//! Notably the app interface allows calling functions exposed by the hApps'
//! modules, called DNAs. To discover a particular hApp's structure, its app
//! info can be requested.

mod admin_interface;
mod app_interface;
pub mod config;
pub mod signal_subscription;
pub mod state_dump;

pub use admin_interface::*;
pub use app_interface::*;
pub use config::*;
pub use state_dump::*;
