use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::ribosome::host_fn::HostFnApi;
use crate::core::ribosome::{CallContext, Invocation, Ribosome, RibosomeImplT};
use crate::prelude::{ExternIO, FunctionName, ZomeName};
use futures::future::BoxFuture;
use holo_hash::{DnaHash, HasHash};
use holochain_zome_types::dna_def::DnaDefHashed;
use holochain_zome_types::prelude::{DynInlineZome, Zome, ZomeDef};
use opentelemetry::KeyValue;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::sync::{Arc, RwLock};

/// Store for inline zomes.
///
/// Mirrors the WASM store, where hashes are stored in the [`DnaDef`] and WASM source code is stored
/// in the database. Since inline zomes cannot be serialized, this store must be populated before
/// the conductor starts and use a UUID for an inline zome instead of a hash.
#[derive(Default)]
pub struct InlineZomeStore {
    inner: Arc<RwLock<HashMap<DnaHash, HashMap<String, DynInlineZome>>>>,
}

impl InlineZomeStore {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub fn insert(&self, dna_hash: DnaHash, zome: DynInlineZome) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(dna_hash)
            .or_insert_with(|| Default::default())
            .insert(zome.uuid(), zome);
    }

    pub fn lookup_zome(&self, dna_def: &DnaDefHashed, zome_name: &ZomeName) -> RibosomeResult<DynInlineZome> {
        let zome = dna_def.get_zome(zome_name)?;

        match zome.def {
            ZomeDef::Wasm(_) => Err(RibosomeError::ZomeTypeMismatch("Expected an inline zome, but got a WASM zome".to_string())),
            ZomeDef::Inline(zome) => {
                self.inner.read().unwrap_or_else(|e| e.into_inner()).get(dna_def.as_hash()).ok_or_else(|| {
                    RibosomeError::ZomeSourceMissing(zome_name.clone())
                })?.get(&zome.uuid).ok_or_else(|| {
                    RibosomeError::ZomeSourceMissing(zome_name.clone())
                }).cloned()
            }
        }
    }
}

#[derive(Clone)]
pub struct InlineRibosome {
    dna_def: DnaDefHashed,

    inline_zome_store: Arc<InlineZomeStore>,
}

impl std::fmt::Debug for InlineRibosome {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InlineRibosome")
            .field("dna_def", &self.dna_def)
            .finish()
    }
}

impl InlineRibosome {
    pub fn new(dna_def: DnaDefHashed, inline_zome_store: Arc<InlineZomeStore>) -> Self {
        Self { dna_def, inline_zome_store }
    }
}

impl RibosomeImplT for InlineRibosome {
    fn maybe_call(
        &self,
        ribosome: Arc<Ribosome>,
        call_context: CallContext,
        invocation: Arc<dyn Invocation + 'static>,
        zome: Zome,
        fn_name: FunctionName,
        _attributes: Vec<KeyValue>,
    ) -> BoxFuture<'static, Result<Option<ExternIO>, RibosomeError>> {
        let zome_res = self.inline_zome_store.lookup_zome(&self.dna_def, zome.zome_name());

        Box::pin(async move {
            let zome = zome_res?;

            let input = invocation
                .take_host_input()?
                .ok_or_else(|| RibosomeError::HostInputMissing)?;
            let api = HostFnApi::new(ribosome, Arc::new(call_context));

            let out = zome.maybe_call(Box::new(api), &fn_name, input)?;

            Ok(out)
        })
    }

    fn call_const_fn(
        &self,
        // TODO this means the inline zomes are accessing the host functions with different inputs
        _ribosome: Arc<Ribosome>,
        zome: Zome,
        name: String,
    ) -> BoxFuture<'_, Result<Option<i32>, RibosomeError>> {
        let zome_res = self.inline_zome_store.lookup_zome(&self.dna_def, zome.zome_name());

        Box::pin(async move {
            let zome = zome_res?;
            Ok(zome.get_global(&name).map(|i| i as i32))
        })
    }

    fn list_zome_fns(&self, zome_name: &ZomeName) -> RibosomeResult<Vec<FunctionName>> {
        let zome = self.inline_zome_store.lookup_zome(&self.dna_def, zome_name)?;

        Ok(zome.functions())
    }
}
