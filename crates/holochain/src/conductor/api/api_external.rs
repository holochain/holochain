use super::error::{ConductorApiResult, SerializationError};
use crate::conductor::{
    interface::error::{AdminInterfaceError, InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holochain_serialized_bytes::prelude::*;
use std::{collections::HashMap, path::PathBuf};
use sx_types::{
    cell::CellHandle,
    dna::Dna,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tracing::*;

#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse>;
}

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.admin(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AdminResponse::Error {
                debug: e.to_string(),
                error_type: e.into(),
            },
        }
    }
}
/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AppInterfaceApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: AppRequest) -> AppResponse {
        let res: ConductorApiResult<AppResponse> = async move {
            match request {
                AppRequest::ZomeInvocationRequest { request } => {
                    Ok(AppResponse::ZomeInvocationResponse {
                        response: Box::new(self.invoke_zome(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => AppResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}

/// The concrete (non-mock) implementation of the AdminInterfaceApi
#[derive(Clone)]
pub struct StdAdminInterfaceApi {
    /// Mutable access to the Conductor
    conductor_handle: ConductorHandle,

    // TODO: I already forget why we needed to put this in here! ~MD
    // To spawn new app APIs you need a copy of the App Api :)
    app_api: StdAppInterfaceApi,
}

impl StdAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        let app_api = StdAppInterfaceApi::new(conductor_handle.clone());
        StdAdminInterfaceApi {
            conductor_handle,
            app_api,
        }
    }

    pub(crate) async fn install_dna(&self, dna_path: PathBuf) -> ConductorApiResult<AdminResponse> {
        trace!(?dna_path);
        let dna = Self::read_parse_dna(dna_path).await?;
        trace!(line = line!());
        self.add_dna(dna).await?;
        Ok(AdminResponse::DnaInstalled)
    }

    async fn add_dna(&self, dna: Dna) -> ConductorApiResult<()> {
        self.conductor_handle
            .write()
            .await
            .fake_dna_cache
            .insert(dna.address(), dna);
        Ok(())
    }

    async fn read_parse_dna(dna_path: PathBuf) -> ConductorApiResult<Dna> {
        let dna: UnsafeBytes = tokio::fs::read(dna_path).await?.into();
        let dna = SerializedBytes::from(dna);
        dna.try_into()
            .map_err(|e| SerializationError::from(e).into())
    }

    async fn list_dnas(&self) -> ConductorApiResult<AdminResponse> {
        let dna_list = self
            .conductor_handle
            .read()
            .await
            .fake_dna_cache
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        Ok(AdminResponse::ListDnas(dna_list))
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for StdAdminInterfaceApi {
    async fn admin(&self, request: AdminRequest) -> ConductorApiResult<AdminResponse> {
        use AdminRequest::*;
        match request {
            Start(_cell_handle) => unimplemented!(),
            Stop(_cell_handle) => unimplemented!(),
            InstallDna(dna_path) => self.install_dna(dna_path).await,
            ListDnas => self.list_dnas().await,
        }
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        match request {
            Ok(request) => Ok(AdminInterfaceApi::handle_request(self, request).await),
            Err(e) => Ok(AdminResponse::Error {
                debug: e.to_string(),
                error_type: InterfaceError::SerializedBytes(e.into()).into(),
            }),
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct StdAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl StdAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for StdAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_handle.read().await;
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        match request {
            Ok(request) => Ok(AppInterfaceApi::handle_request(self, request).await),
            Err(e) => Ok(AppResponse::Error {
                debug: e.to_string(),
            }),
        }
    }
}
/// The set of messages that a conductor understands how to respond
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppResponse {
    Error {
        debug: String,
    },
    ZomeInvocationResponse {
        response: Box<ZomeInvocationResponse>,
    },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminResponse {
    Unimplemented(AdminRequest),
    DnaInstalled,
    ListDnas(Vec<Address>),
    Error {
        debug: String,
        error_type: AdminInterfaceError,
    },
}

/// The set of messages that a conductor understands how to handle
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppRequest {
    CryptoRequest { request: Box<CryptoRequest> },
    TestRequest { request: Box<TestRequest> },
    ZomeInvocationRequest { request: Box<ZomeInvocation> },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminRequest {
    Start(CellHandle),
    Stop(CellHandle),
    InstallDna(PathBuf),
    ListDnas,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum TestRequest {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use matches::assert_matches;
    use sx_types::test_utils::{fake_dna, fake_dna_file};
    use uuid::Uuid;

    #[tokio::test]
    async fn install_list_dna() -> Result<()> {
        let conductor = Conductor::build().test().await?;
        let admin_api = StdAdminInterfaceApi::new(conductor);
        let uuid = Uuid::new_v4();
        let dna = fake_dna(&uuid.to_string());
        let dna_address = dna.address();
        admin_api.add_dna(dna).await?;
        let dna_list = admin_api.list_dnas().await?;
        let expects = vec![dna_address];
        assert_matches!(dna_list, AdminResponse::ListDnas(a) if a == expects);
        Ok(())
    }

    #[tokio::test]
    async fn dna_read_parses() -> Result<()> {
        let uuid = Uuid::new_v4();
        let dna = fake_dna(&uuid.to_string());
        let (dna_path, _tmpdir) = fake_dna_file(dna.clone())?;
        let result = StdAdminInterfaceApi::read_parse_dna(dna_path).await?;
        assert_eq!(dna, result);
        Ok(())
    }
}
