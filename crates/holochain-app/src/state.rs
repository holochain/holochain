use std::collections::HashMap;

use crate::ported::*;

/// Storage for all installed apps
pub trait AppState {
    /// The error type
    type Error;

    /// The collection of all installed apps
    fn apps(&self) -> &Apps;

    /// Getter for a single app. Returns error if app missing.
    fn get_app(&self, id: &AppId) -> Result<&InstalledApp, Self::Error>;

    /// Add an app in the Deactivated state. Returns an error if an app is already
    /// present at the given ID.
    fn add_app(&mut self, app: InstalledAppCommon) -> Result<StoppedApp, Self::Error>;

    /// Remove a single app. Returns error if app missing.
    fn remove_app(&mut self, id: &AppId) -> Result<InstalledApp, Self::Error>;

    /// Update the status of an installed app in-place, as well as a description of
    /// what next action to take.
    fn transition_app_status(
        &mut self,
        id: &AppId,
        transition: AppStatusTransition,
    ) -> Result<(&InstalledApp, AppStatusFx), Self::Error>;
}

/// A collection of apps which can be filtered in various ways
#[derive(derive_more::Deref, derive_more::IntoIterator)]
pub struct Apps(HashMap<AppId, InstalledApp>);

impl Apps {
    /// Iterate over only the "enabled" apps
    pub fn enabled_apps(&self) -> impl Iterator<Item = (&AppId, &InstalledApp)> {
        self.0.iter().filter(|(_, app)| app.status().is_enabled())
    }

    /// Iterate over only the "disabled" apps
    pub fn disabled_apps(&self) -> impl Iterator<Item = (&AppId, &InstalledApp)> {
        self.0.iter().filter(|(_, app)| !app.status().is_enabled())
    }

    /// Iterate over only the "running" apps
    pub fn running_apps(&self) -> impl Iterator<Item = (&AppId, RunningApp)> {
        self.0.iter().filter_map(|(id, app)| {
            if *app.status() == AppStatus::Running {
                let running = RunningApp::from((**app).clone());
                Some((id, running))
            } else {
                None
            }
        })
    }

    /// Iterate over only the paused apps
    pub fn paused_apps(&self) -> impl Iterator<Item = (&AppId, StoppedApp)> {
        self.0.iter().filter_map(|(id, app)| {
            if app.status.is_paused() {
                StoppedApp::from_app(app).map(|stopped| (id, stopped))
            } else {
                None
            }
        })
    }

    /// Iterate over only the "stopped" apps (paused OR disabled)
    pub fn stopped_apps(&self) -> impl Iterator<Item = (&AppId, StoppedApp)> {
        self.0
            .iter()
            .filter_map(|(id, app)| StoppedApp::from_app(app).map(|stopped| (id, stopped)))
    }
}
