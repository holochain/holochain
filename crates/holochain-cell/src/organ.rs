use holochain_dna_types::prelude::*;

/// The status of an installed app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum OrganStatus {
    /// The app is enabled and running normally.
    Running,

    /// Enabled, but stopped due to some recoverable problem.
    /// The app "hopes" to be Running again as soon as possible.
    /// Holochain may restart the app automatically if it can. It may also be
    /// restarted manually via the `StartApp` admin method.
    /// Paused apps will be automatically set to Running when the conductor restarts.
    Paused(PausedOrganReason),

    /// Disabled and stopped, either manually by the user, or automatically due
    /// to an unrecoverable error. App must be Enabled before running again,
    /// and will not restart automaticaly on conductor reboot.
    Disabled(DisabledOrganReason),
}

/// The AppStatus without the reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum OrganStatusKind {
    Running,
    Paused,
    Disabled,
}

impl From<OrganStatus> for OrganStatusKind {
    fn from(status: OrganStatus) -> Self {
        match status {
            OrganStatus::Running => Self::Running,
            OrganStatus::Paused(_) => Self::Paused,
            OrganStatus::Disabled(_) => Self::Disabled,
        }
    }
}

/// Represents a state transition operation from one state to another
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrganStatusTransition {
    /// Attempt to unpause a Paused app
    Start,
    /// Attempt to pause a Running app
    Pause(PausedOrganReason),
    /// Gets an app running no matter what
    Enable,
    /// Disables an app, no matter what
    Disable(DisabledOrganReason),
}

/// The various reasons for why an App is not in the Running state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    derive_more::From,
)]
#[serde(rename_all = "snake_case")]
pub enum StoppedOrganReason {
    /// Same meaning as [`InstalledAppInfoStatus::Paused`](https://docs.rs/holochain_conductor_api/0.0.33/holochain_conductor_api/enum.InstalledAppInfoStatus.html#variant.Paused).
    Paused(PausedOrganReason),

    /// Same meaning as [`InstalledAppInfoStatus::Disabled`](https://docs.rs/holochain_conductor_api/0.0.33/holochain_conductor_api/enum.InstalledAppInfoStatus.html#variant.Disabled).
    Disabled(DisabledOrganReason),
}

impl StoppedOrganReason {
    /// Convert a status into a StoppedAppReason.
    /// If the status is Running, returns None.
    pub fn from_status(status: &OrganStatus) -> Option<Self> {
        match status {
            OrganStatus::Paused(reason) => Some(Self::Paused(reason.clone())),
            OrganStatus::Disabled(reason) => Some(Self::Disabled(reason.clone())),
            OrganStatus::Running => None,
        }
    }
}

impl From<StoppedOrganReason> for OrganStatus {
    fn from(reason: StoppedOrganReason) -> Self {
        match reason {
            StoppedOrganReason::Paused(reason) => Self::Paused(reason),
            StoppedOrganReason::Disabled(reason) => Self::Disabled(reason),
        }
    }
}

/// The reason for an app being in a Paused state.
/// NB: there is no way to manually pause an app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum PausedOrganReason {
    /// The pause was due to a RECOVERABLE error
    Error(String),
}

/// The reason for an app being in a Disabled state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum DisabledOrganReason {
    /// The app is freshly installed, and never started
    NeverStarted,
    /// The disabling was done manually by the user (via admin interface)
    User,
    /// The disabling was due to an UNRECOVERABLE error
    Error(String),
}

impl OrganStatus {
    /// Does this status correspond to an Enabled state?
    /// If false, this indicates a Disabled state.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Running | Self::Paused(_))
    }

    /// Does this status correspond to a Running state?
    /// If false, this indicates a Stopped state.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Does this status correspond to a Paused state?
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused(_))
    }

    /// Transition a status from one state to another.
    /// If None, the transition was not valid, and the status did not change.
    pub fn transition(&mut self, transition: OrganStatusTransition) -> OrganStatusFx {
        use OrganStatus::*;
        use OrganStatusFx::*;
        use OrganStatusTransition::*;
        match (&self, transition) {
            (Running, Pause(reason)) => Some((Paused(reason), SpinDown)),
            (Running, Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Running, Start) | (Running, Enable) => None,

            (Paused(_), Start) => Some((Running, SpinUp)),
            (Paused(_), Enable) => Some((Running, SpinUp)),
            (Paused(_), Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Paused(_), Pause(_)) => None,

            (Disabled(_), Enable) => Some((Running, SpinUp)),
            (Disabled(_), Pause(_)) | (Disabled(_), Disable(_)) | (Disabled(_), Start) => None,
        }
        .map(|(new_status, delta)| {
            *self = new_status;
            delta
        })
        .unwrap_or(NoChange)
    }
}

/// A declaration of the side effects of a particular AppStatusTransition.
///
/// Two values of this type may also be combined into one,
/// to capture the overall effect of a series of transitions.
///
/// The intent of this type is to make sure that any operation which causes an
/// app state transition is followed up with a call to process_app_status_fx
/// in order to reconcile the cell state with the new app state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "be sure to run this value through `process_app_status_fx` to handle any transition effects"]
pub enum OrganStatusFx {
    /// The transition did not result in any change to CellState.
    NoChange,
    /// The transition may cause some Cells to be removed.
    SpinDown,
    /// The transition may cause some Cells to be added (fallibly).
    SpinUp,
    /// The transition may cause some Cells to be removed and some to be (fallibly) added.
    Both,
}

impl Default for OrganStatusFx {
    fn default() -> Self {
        Self::NoChange
    }
}

impl OrganStatusFx {
    /// Combine two effects into one. Think "monoidal append", if that helps.
    pub fn combine(self, other: Self) -> Self {
        use OrganStatusFx::*;
        match (self, other) {
            (NoChange, a) | (a, NoChange) => a,
            (SpinDown, SpinDown) => SpinDown,
            (SpinUp, SpinUp) => SpinUp,
            (Both, _) | (_, Both) => Both,
            (SpinDown, SpinUp) | (SpinUp, SpinDown) => Both,
        }
    }
}
