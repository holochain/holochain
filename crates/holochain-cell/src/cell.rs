use std::collections::HashMap;

use holochain_zome_types::cell::CellId;

pub trait Cell {
    fn status(&self) -> CellStatus;
}

/// The status of an installed Cell, which captures different phases of its lifecycle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellStatus {
    /// Kitsune knows about this Cell and it is considered fully "online"
    Joined,

    /// The Cell is on its way to being fully joined. It is a valid Cell from
    /// the perspective of the conductor, and can handle HolochainP2pEvents,
    /// but it is considered not to be fully running from the perspective of
    /// app status, i.e. if any app has a required Cell with this status,
    /// the app is considered to be in the Paused state.
    PendingJoin(PendingJoinReason),

    /// The Cell is currently in the process of trying to join the network.
    Joining,
}

/// The reason why a cell is waiting to join the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingJoinReason {
    /// The initial state, no attempt has been made to join the network yet.
    Initial,

    /// The join failed with an error that is safe to retry, such as not being connected to the internet.
    Retry,

    /// The network join failed and will not be retried. This will impact the status of the associated
    /// app and require manual intervention from the user.
    Failed,
}
