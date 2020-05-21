use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::DnaDefFixturator;
use crate::fixt::MigrateAgentFixturator;
use fixt::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::DnaDef;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct MigrateAgentInvocation {
    dna_def: DnaDef,
    migrate_agent: MigrateAgent,
}

fixturator!(
    MigrateAgentInvocation,
    {
        let migrate_agent_invocation = MigrateAgentInvocation {
            dna_def: DnaDefFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            migrate_agent: MigrateAgentFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        migrate_agent_invocation
    },
    {
        let migrate_agent_invocation = MigrateAgentInvocation {
            dna_def: DnaDefFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            migrate_agent: MigrateAgentFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        migrate_agent_invocation
    },
    {
        let migrate_agent_invocation = MigrateAgentInvocation {
            dna_def: DnaDefFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            migrate_agent: MigrateAgentFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        migrate_agent_invocation
    }
);

impl Invocation for MigrateAgentInvocation {
    fn allow_side_effects(&self) -> bool {
        false
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "migrate_agent".into(),
            match self.migrate_agent {
                MigrateAgent::Open => "open",
                MigrateAgent::Close => "close",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.migrate_agent).try_into()?))
    }
}

impl TryFrom<MigrateAgentInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(migrate_agent_invocation: MigrateAgentInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&migrate_agent_invocation.migrate_agent).try_into()?,
        ))
    }
}

/// the aggregate result of all zome callbacks for migrating an agent between dnas
#[derive(PartialEq, Debug)]
pub enum MigrateAgentResult {
    /// all implemented migrate agent callbacks in all zomes passed
    Pass,
    /// some migrate agent callback failed
    /// ZomeName is the first zome that failed
    /// String is some human readable string explaining the failure
    Fail(ZomeName, String),
}

impl From<Vec<MigrateAgentCallbackResult>> for MigrateAgentResult {
    fn from(callback_results: Vec<MigrateAgentCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Pass, |acc, x| {
            match x {
                // fail always overrides the acc
                MigrateAgentCallbackResult::Fail(zome_name, fail_string) => {
                    Self::Fail(zome_name, fail_string)
                }
                // pass allows the acc to continue
                MigrateAgentCallbackResult::Pass => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {

    use super::MigrateAgentInvocationFixturator;
    use super::MigrateAgentResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_migrate_agent_unimplemented() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome
            .run_migrate_agent(workspace, migrate_agent_invocation)
            .unwrap();
        assert_eq!(result, MigrateAgentResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_migrate_agent_implemented_pass() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::MigrateAgentPass]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome
            .run_migrate_agent(workspace, migrate_agent_invocation)
            .unwrap();
        assert_eq!(result, MigrateAgentResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_migrate_agent_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::MigrateAgentFail]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome
            .run_migrate_agent(workspace, migrate_agent_invocation)
            .unwrap();
        assert_eq!(
            result,
            MigrateAgentResult::Fail(TestWasm::MigrateAgentFail.into(), "no migrate".into()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_migrate_agent_multi_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![
            TestWasm::MigrateAgentPass,
            TestWasm::MigrateAgentFail,
        ]))
        .next()
        .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome
            .run_migrate_agent(workspace, migrate_agent_invocation)
            .unwrap();
        assert_eq!(
            result,
            MigrateAgentResult::Fail(TestWasm::MigrateAgentFail.into(), "no migrate".into()),
        );
    }
}
