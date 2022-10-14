use holochain_conductor_api::*;
use holochain_websocket::*;

pub struct InterfaceClient {
    ws: WebsocketSender,
    timeout: std::time::Duration,
}

macro_rules! impl_handler {
    ($self: ident , $in: expr => $enum: ident , $res: ident (Box(_))) => {
        match $self
            .ws
            .request_timeout($in, $self.timeout)
            .await
            .map_err(|e| ExternalApiWireError::internal(e))?
        {
            $enum::$res(v) => Ok(*v),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
    ($self: ident , $in: expr => $enum: ident , $res: ident (_)) => {
        match $self
            .ws
            .request_timeout($in, $self.timeout)
            .await
            .map_err(|e| ExternalApiWireError::internal(e))?
        {
            $enum::$res(v) => Ok(v),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
    ($self: ident , $in: expr => $enum: ident , $res: ident) => {
        match $self
            .ws
            .request_timeout($in, $self.timeout)
            .await
            .map_err(|e| ExternalApiWireError::internal(e))?
        {
            $enum::$res => Ok(()),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
}

#[async_trait::async_trait]
impl AppInterface for InterfaceClient {
    async fn app_info(&self, installed_app_id: InstalledAppId) -> Res<Option<InstalledAppInfo>> {
        impl_handler!(
            self, AppRequest::AppInfo { installed_app_id } => AppResponse, AppInfo(_)

        )
    }

    async fn zome_call(&self, call: ZomeCall) -> Res<ZomeCallResponse> {
        impl_handler!(
            self, AppRequest::ZomeCall(Box::new(call)) => AppResponse, ZomeCall(Box(_))
        )
        .map(ZomeCallResponse::Ok)
    }

    async fn create_clone_cell(&self, payload: CreateCloneCellPayload) -> Res<InstalledCell> {
        impl_handler!(
            self, AppRequest::CreateCloneCell(Box::new(payload)) => AppResponse, CloneCellCreated(_)

        )
    }

    async fn archive_clone_cell(&self, payload: ArchiveCloneCellPayload) -> Res<()> {
        impl_handler!(
            self, AppRequest::ArchiveCloneCell(Box::new(payload)) => AppResponse, CloneCellArchived

        )
    }
}

#[async_trait::async_trait]
impl AdminInterface for InterfaceClient {
    async fn update_coordinators(&self, payload: UpdateCoordinatorsPayload) -> Res<()> {
        impl_handler!(
            self, AdminRequest::UpdateCoordinators(Box::new(payload)) => AdminResponse, CoordinatorsUpdated

        )
    }

    async fn install_app(&self, payload: InstallAppPayload) -> Res<InstalledAppInfo> {
        impl_handler!(
            self, AdminRequest::InstallApp(Box::new(payload)) => AdminResponse, AppInstalled(_)

        )
    }

    async fn install_app_bundle(&self, payload: InstallAppBundlePayload) -> Res<InstalledAppInfo> {
        impl_handler!(
            self, AdminRequest::InstallAppBundle(Box::new(payload)) => AdminResponse, AppBundleInstalled(_)

        )
    }

    async fn uninstall_app(&self, installed_app_id: InstalledAppId) -> Res<()> {
        impl_handler!(
            self, AdminRequest::UninstallApp { installed_app_id } => AdminResponse, AppUninstalled

        )
    }

    async fn enable_app(&self, installed_app_id: InstalledAppId) -> Res<AppEnabledResponse> {
        impl_handler!(
            self, AdminRequest::EnableApp { installed_app_id } => AdminResponse, AppEnabled(_)

        )
    }

    async fn disable_app(&self, installed_app_id: InstalledAppId) -> Res<()> {
        impl_handler!(
            self, AdminRequest::DisableApp { installed_app_id } => AdminResponse, AppDisabled

        )
    }

    async fn start_app(&self, installed_app_id: InstalledAppId) -> Res<bool> {
        impl_handler!(
            self, AdminRequest::StartApp { installed_app_id } => AdminResponse, AppStarted(_)

        )
    }
}
