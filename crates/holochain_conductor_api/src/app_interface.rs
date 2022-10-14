#![allow(deprecated)]

use crate::AppEnabledResponse;
use crate::{signal_subscription::SignalSubscription, ExternalApiWireError};
use holo_hash::AgentPubKey;
use holochain_types::prelude::*;
use holochain_types::prelude::{
    ArchiveCloneCellPayload, CreateCloneCellPayload, InstalledAppId, InstalledCell,
};
use holochain_types::prelude::{
    InstallAppBundlePayload, InstallAppPayload, UpdateCoordinatorsPayload,
};
use holochain_zome_types::ZomeCallResponse;

pub type Res<T> = Result<T, ExternalApiWireError>;

#[async_trait::async_trait]
pub trait AppInterface {
    async fn app_info(&self, installed_app_id: InstalledAppId) -> Res<Option<InstalledAppInfo>>;

    async fn zome_call(&self, call: ZomeCall) -> Res<ZomeCallResponse>;

    async fn create_clone_cell(&self, payload: CreateCloneCellPayload) -> Res<InstalledCell>;

    async fn archive_clone_cell(&self, payload: ArchiveCloneCellPayload) -> Res<()>;
}

#[async_trait::async_trait]
pub trait AdminInterface {
    async fn update_coordinators(&self, payload: UpdateCoordinatorsPayload) -> Res<()>;

    async fn install_app(&self, payload: InstallAppPayload) -> Res<InstalledAppInfo>;

    async fn install_app_bundle(&self, payload: InstallAppBundlePayload) -> Res<InstalledAppInfo>;

    async fn uninstall_app(&self, id: InstalledAppId) -> Res<()>;

    async fn enable_app(&self, id: InstalledAppId) -> Res<AppEnabledResponse>;

    async fn disable_app(&self, id: InstalledAppId) -> Res<()>;

    async fn start_app(&self, id: InstalledAppId) -> Res<bool>;
}

/// Represents the available conductor functions to call over an app interface
/// and will result in a corresponding [`AppResponse`] message being sent back over the
/// interface connection.
///
/// # Errors
///
/// Returns an [`AppResponse::Error`] with a reason why the request failed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Get info about the app identified by the given `installed_app_id` argument,
    /// including info about each cell installed by this app.
    ///
    /// Requires `installed_app_id`, because an app interface can be the interface to multiple
    /// apps at the same time.
    ///
    /// # Returns
    ///
    /// [`AppResponse::AppInfo`]
    AppInfo {
        /// The app ID for which to get information
        installed_app_id: InstalledAppId,
    },
    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`].
    Crypto(Box<CryptoRequest>),
    /// Call a zome function. See [`ZomeCall`]
    /// to understand the data that must be provided.
    ///
    /// # Returns
    ///
    /// [`AppResponse::ZomeCall`]
    ZomeCall(Box<ZomeCall>),

    /// Clone a DNA (in the biological sense), thus creating a new `Cell`.
    ///
    /// Using the provided, already-registered DNA, create a new DNA with a unique
    /// ID and the specified properties, create a new cell from this cloned DNA,
    /// and add the cell to the specified app.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellCreated`]
    CreateCloneCell(Box<CreateCloneCellPayload>),

    /// Archive a clone cell.
    ///
    /// Providing a [`CloneId`] or [`CellId`], archive an existing clone cell.
    /// When the clone cell exists, it is archived and can not be called any
    /// longer. If it doesn't exist, the call is a no-op.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellArchived`] if the clone cell existed
    /// and was archived.
    ArchiveCloneCell(Box<ArchiveCloneCellPayload>),

    #[deprecated = "use ZomeCall"]
    ZomeCallInvocation(Box<ZomeCall>),

    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`].
    SignalSubscription(SignalSubscription),
}

/// Represents the possible responses to an [`AppRequest`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppResponse {
    /// This request is unimplemented
    Unimplemented(AppRequest),

    /// Can occur in response to any [`AppRequest`].
    ///
    /// There has been an error during the handling of the request.
    Error(ExternalApiWireError),

    /// The succesful response to an [`AppRequest::AppInfo`].
    ///
    /// Option will be `None` if there is no installed app with the given `installed_app_id`.
    /// Check out [`InstalledApp`] for details on when the option is `Some<InstalledAppInfo>`
    AppInfo(Option<InstalledAppInfo>),

    /// The successful response to an [`AppRequest::ZomeCall`].
    ///
    /// Note that [`ExternIO`] is simply a structure of [`struct@SerializedBytes`], so the client will have
    /// to decode this response back into the data provided by the zome using a [msgpack] library to utilize it.
    ///
    /// [msgpack]: https://msgpack.org/
    ZomeCall(Box<ExternIO>),

    /// The successful response to an [`AppRequest::CreateCloneCell`].
    ///
    /// The response contains an [`InstalledCell`] with the created clone
    /// cell's [`CloneId`] and [`CellId`].
    CloneCellCreated(InstalledCell),

    /// An existing clone cell has been archived.
    CloneCellArchived,

    #[deprecated = "use ZomeCall"]
    ZomeCallInvocation(Box<ExternIO>),
}

/// The data provided over an app interface in order to make a zome call
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCall {
    /// The ID of the cell containing the zome to be called
    pub cell_id: CellId,
    /// The zome containing the function to be called
    pub zome_name: ZomeName,
    /// The name of the zome function to call
    pub fn_name: FunctionName,
    /// The serialized data to pass as an argument to the zome function call
    pub payload: ExternIO,
    /// The capability request authorization
    ///
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an `Unrestricted` status
    /// via a `CapGrant`. Otherwise it will be necessary to provide a `CapSecret` for every call.
    pub cap_secret: Option<CapSecret>,
    /// The provenance (source) of the call
    ///
    /// NB: **This will be removed** as soon as Holochain has a way of determining who
    /// is making this zome call over this interface. Until we do, the caller simply
    /// provides this data and Holochain trusts them.
    pub provenance: AgentPubKey,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// Info about an installed app, returned as part of [`AppResponse::AppInfo`]
pub struct InstalledAppInfo {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,
    /// Info about the cells installed in this app
    pub cell_data: Vec<InstalledCell>,
    /// The app's current status, in an API-friendly format
    pub status: InstalledAppInfoStatus,
}

impl InstalledAppInfo {
    pub fn from_installed_app(app: &InstalledApp) -> Self {
        let installed_app_id = app.id().clone();
        let status = app.status().clone().into();
        let clone_cells = app
            .clone_cells()
            .map(|cell| (cell.0.as_app_role_id(), cell.1));
        let cells = app.provisioned_cells().chain(clone_cells);
        let cell_data = cells
            .map(|(role_id, id)| InstalledCell::new(id.clone(), role_id.clone()))
            .collect();
        Self {
            installed_app_id,
            cell_data,
            status,
        }
    }
}

impl From<&InstalledApp> for InstalledAppInfo {
    fn from(app: &InstalledApp) -> Self {
        Self::from_installed_app(app)
    }
}

/// A flat, slightly more API-friendly representation of [`InstalledAppInfo`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum InstalledAppInfoStatus {
    Paused { reason: PausedAppReason },
    Disabled { reason: DisabledAppReason },
    Running,
}

impl From<AppStatus> for InstalledAppInfoStatus {
    fn from(i: AppStatus) -> Self {
        match i {
            AppStatus::Running => InstalledAppInfoStatus::Running,
            AppStatus::Disabled(reason) => InstalledAppInfoStatus::Disabled { reason },
            AppStatus::Paused(reason) => InstalledAppInfoStatus::Paused { reason },
        }
    }
}

impl From<InstalledAppInfoStatus> for AppStatus {
    fn from(i: InstalledAppInfoStatus) -> Self {
        match i {
            InstalledAppInfoStatus::Running => AppStatus::Running,
            InstalledAppInfoStatus::Disabled { reason } => AppStatus::Disabled(reason),
            InstalledAppInfoStatus::Paused { reason } => AppStatus::Paused(reason),
        }
    }
}

#[test]
fn status_serialization() {
    use kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::serde_json;

    let status: InstalledAppInfoStatus =
        AppStatus::Disabled(DisabledAppReason::Error("because".into())).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"disabled\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: InstalledAppInfoStatus =
        AppStatus::Paused(PausedAppReason::Error("because".into())).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"paused\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: InstalledAppInfoStatus = AppStatus::Disabled(DisabledAppReason::User).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"disabled\":{\"reason\":\"user\"}}"
    );
}
