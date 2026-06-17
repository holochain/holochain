use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::{CallContext, Invocation, MockRibosomeImplT, Ribosome};
use hdk::prelude::{DnaModifiers, ValidateCallbackResult};
use holochain_zome_types::dna_def::{DnaDefBuilder, DnaDefHashed};
use holochain_zome_types::info::NetworkSeed;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::prelude::FunctionName;
use holochain_zome_types::zome_io::ExternIO;
use mockall::predicate::{always, eq};
use std::sync::Arc;

pub struct MockRibosomeBuilder {
    mock: MockRibosomeImplT,

    dna_def: DnaDefHashed,

    num_entry_types: i32,

    num_link_types: i32,
}

impl Default for MockRibosomeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRibosomeBuilder {
    pub fn new() -> Self {
        let dna_def = DnaDefBuilder::default()
            .modifiers(DnaModifiers {
                network_seed: NetworkSeed::default(),
                properties: ().try_into().unwrap(),
            })
            .integrity_zomes(vec![])
            .coordinator_zomes(vec![])
            .build()
            .unwrap();
        Self::new_with_dna_def(DnaDefHashed::from_content_sync(dna_def))
    }

    pub fn new_with_dna_def(dna_def: DnaDefHashed) -> Self {
        Self {
            mock: MockRibosomeImplT::default(),
            dna_def,
            num_entry_types: 0,
            num_link_types: 0,
        }
    }

    pub fn with_num_entry_types(mut self, num_entry_types: i32) -> Self {
        self.num_entry_types = num_entry_types;
        self
    }

    pub fn with_num_link_types(mut self, num_link_types: i32) -> Self {
        self.num_link_types = num_link_types;
        self
    }

    pub fn with_init_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<InitCallbackResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("init".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    pub fn with_validate_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<ValidateCallbackResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("validate".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    pub fn with_genesis_self_check_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<GenesisSelfCheckResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("genesis_self_check_1".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    pub fn with_post_commit_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<()>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("post_commit".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
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

    pub async fn build(mut self) -> RibosomeResult<Ribosome> {
        self.mock
            .expect_call_const_fn()
            .with(always(), always(), eq::<String>("__num_entry_types".into()))
            .returning(move |_, _, _| {
                let num_entry_types = self.num_entry_types;
                Box::pin(async move { Ok(Some(num_entry_types)) })
            });

        self.mock
            .expect_call_const_fn()
            .with(always(), always(), eq::<String>("__num_link_types".into()))
            .returning(move |_, _, _| {
                let num_link_types = self.num_link_types;
                Box::pin(async move { Ok(Some(num_link_types)) })
            });

        // After all other expectations have been added, provide a maybe_call expectation that
        // handles missing functions
        self.mock
            .expect_maybe_call()
            .with(always(), always(), always(), always(), always(), always())
            .returning(move |_, _, _, _, _, _| Box::pin(async move { Ok(None) }));

        Ribosome::new(self.dna_def, self.mock).await
    }
}
