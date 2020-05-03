//! the _host_ types used to track the status/result of migrating an agent
//! c.f. _guest_ types for migrate agent callbacks across the wasm boudary in zome_types

use crate::nucleus::ZomeName;

/// the aggregate result of all zome callbacks for migrating an agent between dnas
pub enum MigrateAgentResult {
    /// all implemented migrate agent callbacks in all zomes passed
    Pass,
    /// some migrate agent callback failed
    /// ZomeName is the first zome that failed
    /// String is some human readable string explaining the failure
    Fail(ZomeName, String),
}
