use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::ribosome::host_fn::HostFnApi;
use crate::core::ribosome::{CallContext, Invocation, Ribosome, RibosomeT};
use crate::prelude::{ExternIO, FunctionName, ZomeIndex, ZomeName};
use futures::future::BoxFuture;
use holo_hash::{DnaHash, HasHash};
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::dna_def::DnaDefHashed;
use holochain_zome_types::prelude::{DynInlineZome, InlineZomeT, IntegrityZome, Zome};
use opentelemetry::KeyValue;
use std::fmt::Formatter;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct InlineZomeDef {
    inline_zome: DynInlineZome,
}

#[derive(Clone)]
pub struct InlineRibosome {
    dna_def: DnaDefHashed,

    zomes: InlineZomeSet,
}

impl std::fmt::Debug for InlineRibosome {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InlineRibosome")
            .field("dna_def", &self.dna_def)
            .finish()
    }
}

impl InlineRibosome {
    pub fn new(dna_def: DnaDefHashed, zomes: InlineZomeSet) -> Self {
        Self { dna_def, zomes }
    }

    fn zome_by_name(&self, zome_name: &ZomeName) -> RibosomeResult<Arc<dyn InlineZomeT + Send + Sync>> {
        self.zomes.integrity_zomes
            .get(zome_name.0.as_ref())
            .map(|z| Arc::new(z.clone()) as Arc<dyn InlineZomeT + Send + Sync>)
            .or_else(|| self.zomes.coordinator_zomes
                .get(zome_name.0.as_ref())
                .map(|z| Arc::new(z.clone()) as Arc<dyn InlineZomeT + Send + Sync>))
            .ok_or_else(|| RibosomeError::ZomeNotExists(zome_name.clone()))
    }
}

impl RibosomeT for InlineRibosome {
    fn dna_def_hashed(&self) -> &DnaDefHashed {
        &self.dna_def
    }

    fn dna_hash(&self) -> &DnaHash {
        self.dna_def.as_hash()
    }

    fn maybe_call(
        &self,
        ribosome: Arc<Ribosome>,
        call_context: CallContext,
        invocation: Arc<dyn Invocation + 'static>,
        zome: Zome,
        fn_name: FunctionName,
        _attributes: Vec<KeyValue>,
    ) -> BoxFuture<'static, Result<Option<ExternIO>, RibosomeError>>
    {
        let func = self.zome_by_name(zome.zome_name());

        Box::pin(async move {
            let func = func?;
            let input = invocation.take_host_input()?.ok_or_else(|| {
                RibosomeError::HostInputMissing
            })?;
            let api = HostFnApi::new(ribosome, Arc::new(call_context));

            let out = func.maybe_call(Box::new(api), &fn_name, input)?;

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
        let zome = self.zome_by_name(zome.zome_name());
        Box::pin(async move {
            let zome = zome?;
            Ok(zome.get_global(&name).map(|i| i as i32))
        })
    }

    fn list_zome_fns(&self, zome_name: &ZomeName) -> RibosomeResult<Vec<FunctionName>> {
        self.zomes.integrity_zomes.get(zome_name.0.as_ref()).map(|z| z.functions()).or_else(|| {
            self.zomes.coordinator_zomes.get(zome_name.0.as_ref()).map(|z| z.functions())
        }).ok_or_else(|| {
            RibosomeError::ZomeNotExists(zome_name.clone())
        })
    }

    // fn zome_types(&self) -> &Arc<GlobalZomeTypes> {
    //     todo!()
    // }
}
