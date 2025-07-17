//! # Additional Documentation
//!
//! ## Enum Serialization Convention
//!
//! Serialization of enums exposed on the conductor API in most cases follow the following
//! convention:
//!
//! 1. Enums with **only unit-like variants** have their variant names converted to `snake_case`
//! using the `#[serde(rename_all = "snake_case")]` attribute.
//!
//! For example:
//!
//! ```ignore
//! #[serde(rename_all = "snake_case")]
//! pub enum AppStatusFilter {
//!     Enabled,
//!     Disabled,
//! }
//! ```
//!
//! would lead to the following associated TypeScript type
//!
//! ```ignore
//! type AppStatusFilter = "enabled" | "disabled";
//! ```
//!
//! 2. Enums that **include tuple-like and/or struct-like variants** are serialized using
//! the `#[serde(tag = "type", content = "value", rename_all = "snake_case")]` attributes.
//!
//! For example:
//!
//! ```ignore
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
//! would lead to the following associated TypeScript type
//!
//! ```ignore
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
