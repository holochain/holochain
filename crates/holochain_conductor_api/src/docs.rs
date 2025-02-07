//! # Additional Documentation
//!
//! ## Enum Serialization Convention
//!
//! Serialization of enums exposed on the conductor API in most cases follow the following
//! convention:
//!
//! 1. enums with **only unit-like variants** have their variant names converted to `snake_case`
//! using the `#[serde(rename_all = "snake_case")]` attribute.
//!
//! For example:
//!
//! ```
//! #[serde(rename_all = "snake_case")]
//! pub enum AppStatusFilter {
//!     Enabled,
//!     Disabled,
//!     Running,
//!     Stopped,
//!     Paused,
//! }
//! ```
//!
//! would lead to the following associated typescript type
//!
//! ```
//! type AppStatusFilter = "enabled" | "disabled" | "running" | "stopped" | "paused";
//! ```
//!
//! 2. enums that **include tuple-like and/or struct-like variants** are serialized using
//! the `#[serde(tag = "type", content = "value", rename_all = "snake_case")]` attributes.
//!
//! For example:
//!
//! ```
//! #[serde(tag = "type", content = "value", rename_all = "snake_case")]
//! pub enum Signal {
//!     App {
//!         cell_id: CellId,
//!         zome_name: ZomeName,
//!         signal: AppSignal,
//!     },
//!     System(SystemSignal),
//! }
//! ```
//!
//! would lead to the following associated typescript type
//!
//! ```
//! type Signal =
//!   | {
//!       type: "app",
//!       value: {
//!         cell_id: CellId,
//!         zome_name: ZomeName,
//!         signal: AppSignal
//!       }
//!     }
//!   | {
//!       type: "system_signal"
//!       value: SystemSignal
//!     };
//! ```
