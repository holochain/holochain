use crate::core::ribosome::{CallContext, HostContext, Ribosome, RibosomeError};
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

/// Read the init properties supplied for this cell's role at install time.
///
/// Restricted to the `init` callback. The properties are resolved from the
/// conductor database for the app and role that this cell is provisioned for.
/// Returns `None` when no init properties were supplied for the role.
pub fn get_init_properties(
    _ribosome: Arc<Ribosome>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<Option<InitProperties>, RuntimeError> {
    match call_context.host_context() {
        HostContext::Init(_) => {
            let call_zome_handle = call_context.host_context().call_zome_handle().clone();
            let properties = tokio_helper::block_forever_on(async move {
                call_zome_handle.get_init_properties().await
            })
            .map_err(|conductor_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
            })?;
            Ok(properties)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_init_properties".into()
            )
            .to_string()
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::api::MockCellConductorReadHandleT;
    use crate::core::ribosome::mock_ribosome::MockRibosomeBuilder;
    use crate::fixt::{
        CallContextFixturator, InitHostAccessFixturator, ZomeCallHostAccessFixturator,
    };
    use ::fixt::prelude::*;

    /// In an `init` context the properties resolved from the conductor are returned.
    #[tokio::test(flavor = "multi_thread")]
    async fn get_init_properties_returns_resolved_properties() {
        let ribosome = MockRibosomeBuilder::new().build().await.unwrap();
        let expected = InitProperties(SerializedBytes::from(UnsafeBytes::from(vec![1, 2, 3])));

        let mut handle = MockCellConductorReadHandleT::new();
        let returned = expected.clone();
        handle.expect_get_init_properties().returning(move || {
            let returned = returned.clone();
            Box::pin(async move { Ok(Some(returned)) })
        });

        let mut host_access = InitHostAccessFixturator::new(Empty).next().unwrap();
        host_access.call_zome_handle = Arc::new(handle);

        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.host_context = host_access.into();

        let output = get_init_properties(Arc::new(ribosome), Arc::new(call_context), ()).unwrap();

        assert_eq!(output, Some(expected));
    }

    /// An `init` context with no properties stored for the role returns `None`.
    #[tokio::test(flavor = "multi_thread")]
    async fn get_init_properties_returns_none_when_unset() {
        let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

        let mut handle = MockCellConductorReadHandleT::new();
        handle
            .expect_get_init_properties()
            .returning(|| Box::pin(async { Ok(None) }));

        let mut host_access = InitHostAccessFixturator::new(Empty).next().unwrap();
        host_access.call_zome_handle = Arc::new(handle);

        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.host_context = host_access.into();

        let output = get_init_properties(Arc::new(ribosome), Arc::new(call_context), ()).unwrap();

        assert_eq!(output, None);
    }

    /// Calling `get_init_properties` outside of an `init` context is denied.
    #[tokio::test(flavor = "multi_thread")]
    async fn get_init_properties_denied_outside_init() {
        let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

        // A plain zome-call host context is not an `init` context.
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.host_context = host_access.into();

        let result = get_init_properties(Arc::new(ribosome), Arc::new(call_context), ());

        assert!(result.is_err());
    }
}
