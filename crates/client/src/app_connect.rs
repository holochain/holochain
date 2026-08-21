use crate::error::{ConductorApiError, ConductorApiResult};
use crate::AdminWebsocket;
use holochain_types::app::InstalledAppId;

/// Finds the port of an app interface that accepts `installed_app_id` from
/// `origin`.
///
/// An interface qualifies when it is either unrestricted or bound to this app,
/// and its allowed origins admit `origin`. Where several qualify, the lowest
/// port is chosen so that repeated calls against an unchanged conductor agree.
pub async fn discover_app_interface_port(
    admin_ws: &AdminWebsocket,
    installed_app_id: &InstalledAppId,
    origin: Option<&str>,
) -> ConductorApiResult<u16> {
    let interfaces = admin_ws.list_app_interfaces().await?;

    interfaces
        .iter()
        .filter(|interface| match &interface.installed_app_id {
            Some(bound) => bound == installed_app_id,
            None => true,
        })
        .filter(|interface| match origin {
            Some(origin) => interface.allowed_origins.is_allowed(origin),
            None => true,
        })
        .map(|interface| interface.port)
        .min()
        .ok_or_else(|| ConductorApiError::AppInterfaceNotFound {
            installed_app_id: installed_app_id.clone(),
            origin: origin.map(str::to_string),
        })
}
