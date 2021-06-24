use crate::{signal_subscription::SignalSubscription, ExternalApiWireError};
use holo_hash::AgentPubKey;
use holochain_types::prelude::*;

/// Represents the available Conductor functions to call over an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Get info about the App identified by the given `installed_app_id` argument,
    /// including info about each Cell installed by this App.
    /// Requires `installed_app_id` because an App interface can be the interface to multiple
    /// apps at the same time.
    ///
    /// Will be responded to with an [`AppResponse::AppInfo`]
    /// or an [`AppResponse::Error`]
    ///
    /// [`AppResponse::AppInfo`]: enum.AppResponse.html#variant.AppInfo
    /// [`AppResponse::Error`]: enum.AppResponse.html#variant.Error
    AppInfo {
        /// The InstalledAppId for which to get information
        installed_app_id: InstalledAppId,
    },
    /// Asks the conductor to do some crypto.
    ///
    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`](enum.AppResponse.html#variant.Unimplemented)
    Crypto(Box<CryptoRequest>),
    /// Call a zome function. See the inner [`ZomeCall`]
    /// struct to understand the data that must be provided.
    ///
    /// Will be responded to with an [`AppResponse::ZomeCall`]
    /// or an [`AppResponse::Error`]
    ///
    /// [`ZomeCall`]: ../../core/ribosome/struct.ZomeCall.html
    /// [`AppResponse::ZomeCall`]: enum.AppResponse.html#variant.ZomeCall
    /// [`AppResponse::Error`]: enum.AppResponse.html#variant.Error
    ZomeCall(Box<ZomeCall>),

    /// DEPRECATED. Use `ZomeCall`.
    ZomeCallInvocation(Box<ZomeCall>),

    /// Update signal subscriptions.
    ///
    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`](enum.AppResponse.html#variant.Unimplemented)
    SignalSubscription(SignalSubscription),
}

/// Responses to requests received on an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppResponse {
    /// This request/response is unimplemented
    Unimplemented(AppRequest),

    /// Can occur in response to any [`AppRequest`].
    ///
    /// There has been an error during the handling of the request.
    /// See [`ExternalApiWireError`] for variants.
    ///
    /// [`AppRequest`]: enum.AppRequest.html
    /// [`ExternalApiWireError`]: error/enum.ExternalApiWireError.html
    Error(ExternalApiWireError),

    /// The succesful response to an [`AppRequest::AppInfo`].
    ///
    /// Option will be `None` if there is no installed app with the given `installed_app_id` value from the request.
    /// Check out [`InstalledApp`] for details on when the Option is `Some<InstalledAppInfo>`
    ///
    /// [`InstalledApp`]: ../../../holochain_types/app/struct.InstalledApp.html
    /// [`AppRequest::AppInfo`]: enum.AppRequest.html#variant.AppInfo
    AppInfo(Option<InstalledAppInfo>),

    /// The successful response to an [`AppRequest::ZomeCall`].
    ///
    /// Note that [`ExternIO`] is simply a structure of [`SerializedBytes`] so the client will have
    /// to decode this response back into the data provided by the Zome using a [msgpack](https://msgpack.org/) library to utilize it.
    ///
    /// [`AppRequest::ZomeCall`]: enum.AppRequest.html#variant.ZomeCall
    /// [`ExternIO`]: ../../../holochain_zome_types/zome_io/struct.ExternIO.html
    /// [`SerializedBytes`]: ../../../holochain_zome_types/query/struct.SerializedBytes.html
    ZomeCall(Box<ExternIO>),

    /// DEPRECATED. See `ZomeCall`.
    ZomeCallInvocation(Box<ExternIO>),
}

/// The data provided across an App interface in order to make a zome call
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCall {
    /// The Id of the `Cell` containing the Zome to be called
    pub cell_id: CellId,
    /// The Zome containing the function to be called
    pub zome_name: ZomeName,
    /// The name of the Zome function to call
    pub fn_name: FunctionName,
    /// The serialized data to pass as an argument to the Zome call
    pub payload: ExternIO,
    /// The capability request authorization.
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an Unrestricted status
    /// via a `CapGrant`. Otherwise, it will be necessary to provide a `CapSecret` for every call.
    pub cap: Option<CapSecret>,
    /// The provenance (source) of the call.
    ///
    /// NB: **This will go away** as soon as Holochain has a way of determining who
    /// is making this ZomeCall over this interface. Until we do, the caller simply
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
    /// Info about the Cells installed in this app
    pub cell_data: Vec<InstalledCell>,
    /// The app's current status, in an API-friendly format
    pub status: InstalledAppInfoStatus,
}

impl InstalledAppInfo {
    pub fn from_installed_app(app: &InstalledApp) -> Self {
        let installed_app_id = app.id().clone();
        let status = app.status().clone().into();
        let cell_data = app
            .provisioned_cells()
            .map(|(nick, id)| InstalledCell::new(id.clone(), nick.clone()))
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

/// A flatter, more API-friendly representation of [`InstalledAppStatus`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum InstalledAppInfoStatus {
    NeverStarted,
    Paused { reason: PausedAppReason },
    Disabled { reason: DisabledAppReason },
    Running,
}

impl From<InstalledAppStatus> for InstalledAppInfoStatus {
    fn from(i: InstalledAppStatus) -> Self {
        match i {
            InstalledAppStatus::Running => InstalledAppInfoStatus::Running,
            InstalledAppStatus::Stopped(s) => match s {
                StoppedAppReason::Disabled(reason) => InstalledAppInfoStatus::Disabled { reason },
                StoppedAppReason::Paused(reason) => InstalledAppInfoStatus::Paused { reason },
                StoppedAppReason::NeverStarted => InstalledAppInfoStatus::NeverStarted,
            },
        }
    }
}

#[test]
fn status_serialization() {
    use kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::serde_json;

    let status: InstalledAppInfoStatus = InstalledAppStatus::Stopped(StoppedAppReason::Disabled(
        DisabledAppReason::Error("because".into()),
    ))
    .into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"disabled\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: InstalledAppInfoStatus = InstalledAppStatus::Stopped(StoppedAppReason::Paused(
        PausedAppReason::Error("because".into()),
    ))
    .into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"paused\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: InstalledAppInfoStatus =
        InstalledAppStatus::Stopped(StoppedAppReason::Paused(PausedAppReason::User)).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"paused\":{\"reason\":\"user\"}}"
    );
}
