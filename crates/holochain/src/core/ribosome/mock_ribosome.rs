use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::{CallContext, Invocation, MockRibosomeImplT, Ribosome};
use hdk::prelude::{DnaModifiers, ValidateCallbackResult};
use holochain_zome_types::dna_def::{DnaDefBuilder, DnaDefHashed};
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::prelude::FunctionName;
use holochain_zome_types::zome_io::ExternIO;
use mockall::predicate::{always, eq};
use std::sync::Arc;
use holochain_zome_types::info::NetworkSeed;

pub struct MockRibosomeBuilder {
    mock: MockRibosomeImplT,

    dna_def: DnaDefHashed,
}

impl MockRibosomeBuilder {
    pub fn new() -> Self {
        let dna_def = DnaDefBuilder::default().modifiers(DnaModifiers {
            network_seed: NetworkSeed::default(),
            properties: ().try_into().unwrap(),
        }).build().unwrap();
        Self::new_with_dna_def(DnaDefHashed::from_content_sync(dna_def))
    }

    pub fn new_with_dna_def(dna_def: DnaDefHashed) -> Self {
        Self {
            mock: MockRibosomeImplT::default(),
            dna_def,
        }
    }

    pub fn with_init_handler(mut self, handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<InitCallbackResult> + Clone + Send + Sync + 'static) -> Self {
        self.mock.expect_maybe_call().with(always(), always(), always(), always(), eq::<FunctionName>("init".into()), always()).returning(move |_, call, inv, _, _, _| {
            let mut handler = handler.clone();
            Box::pin(async move {
                handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
            })
        });
        self
    }

    pub fn with_validate_handler(mut self, handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<ValidateCallbackResult> + Clone + Send + Sync + 'static) -> Self {
        self.mock.expect_maybe_call().with(always(), always(), always(), always(), eq::<FunctionName>("validate".into()), always()).returning(move |_, call, inv, _, _, _| {
            let mut handler = handler.clone();
            Box::pin(async move {
                handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
            })
        });
        self
    }

    pub fn with_genesis_self_check_handler(mut self, handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<GenesisSelfCheckResult> + Clone + Send + Sync + 'static) -> Self {
        self.mock.expect_maybe_call().with(always(), always(), always(), always(), eq::<FunctionName>("genesis_self_check".into()), always()).returning(move |_, call, inv, _, _, _| {
            let mut handler = handler.clone();
            Box::pin(async move {
                handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
            })
        });
        self
    }

    pub fn raw_mock(&mut self) -> &mut MockRibosomeImplT {
        &mut self.mock
    }

    pub async fn build(self) -> RibosomeResult<Ribosome> {
        Ribosome::new(self.dna_def, self.mock).await
    }
}
