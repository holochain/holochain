use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::workflow::CallZomeWorkspaceLock;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;

#[derive(Clone)]
pub struct MigrateAgentInvocation {
    dna_def: DnaDef,
    migrate_agent: MigrateAgent,
}

impl MigrateAgentInvocation {
    pub fn new(dna_def: DnaDef, migrate_agent: MigrateAgent) -> Self {
        Self {
            dna_def,
            migrate_agent,
        }
    }
}

#[derive(Clone, Constructor)]
pub struct MigrateAgentHostAccess {
    pub workspace: CallZomeWorkspaceLock,
}

impl From<MigrateAgentHostAccess> for HostAccess {
    fn from(migrate_agent_host_access: MigrateAgentHostAccess) -> Self {
        Self::MigrateAgent(migrate_agent_host_access)
    }
}

impl From<&MigrateAgentHostAccess> for HostFnAccess {
    fn from(_: &MigrateAgentHostAccess) -> Self {
        let mut access = Self::none();
        // TODO: insert zome_name
        access.read_workspace = Permission::Allow;
        access.agent_info = Permission::Allow;
        access.dna_bindings = Permission::Allow;
        access
    }
}

impl Invocation for MigrateAgentInvocation {
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
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(&self.migrate_agent)
    }
}

impl TryFrom<MigrateAgentInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(migrate_agent_invocation: MigrateAgentInvocation) -> Result<Self, Self::Error> {
        ExternIO::encode(&migrate_agent_invocation.migrate_agent)
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

impl From<Vec<(ZomeName, MigrateAgentCallbackResult)>> for MigrateAgentResult {
    fn from(callback_results: Vec<(ZomeName, MigrateAgentCallbackResult)>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Pass, |acc, (zome_name, x)| {
                match x {
                    // fail always overrides the acc
                    MigrateAgentCallbackResult::Fail(fail_string) => {
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
    use super::MigrateAgentResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::MigrateAgentFixturator;
    use crate::fixt::MigrateAgentHostAccessFixturator;
    use crate::fixt::MigrateAgentInvocationFixturator;
    use crate::fixt::ZomeNameFixturator;
    use holochain_types::dna::zome::HostFnAccess;
    use holochain_types::prelude::*;
    use rand::prelude::*;

    #[test]
    fn migrate_agent_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_pass = || MigrateAgentResult::Pass;
        let result_fail = || {
            MigrateAgentResult::Fail(
                ZomeNameFixturator::new(::fixt::Empty).next().unwrap(),
                "".into(),
            )
        };

        let cb_pass = || {
            (
                ZomeNameFixturator::new(::fixt::Empty).next().unwrap(),
                MigrateAgentCallbackResult::Pass,
            )
        };
        let cb_fail = || {
            (
                ZomeNameFixturator::new(::fixt::Empty).next().unwrap(),
                MigrateAgentCallbackResult::Fail("".into()),
            )
        };

        for (mut results, expected) in vec![
            (vec![], result_pass()),
            (vec![cb_pass()], result_pass()),
            (vec![cb_fail()], result_fail()),
            (vec![cb_fail(), cb_pass()], result_fail()),
        ] {
            // order of the results should not change the final result
            results.shuffle(&mut rng);

            // number of times a callback result appears should not change the final result
            let number_of_extras = rng.gen_range(0, 5);
            for _ in 0..number_of_extras {
                let maybe_extra = results.choose(&mut rng).cloned();
                match maybe_extra {
                    Some(extra) => results.push(extra),
                    _ => {}
                };
            }

            assert_eq!(expected, results.into(),);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn migrate_agent_invocation_allow_side_effects() {
        use holochain_types::dna::zome::Permission::*;
        let migrate_agent_host_access =
            MigrateAgentHostAccessFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();
        assert_eq!(
            HostFnAccess::from(&migrate_agent_host_access),
            HostFnAccess {
                agent_info: Allow,
                read_workspace: Allow,
                write_workspace: Deny,
                non_determinism: Deny,
                write_network: Deny,
                dna_bindings: Allow,
                keystore: Deny,
            }
        );
    }

    #[test]
    fn migrate_agent_invocation_zomes() {
        let migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::All, migrate_agent_invocation.zomes(),);
    }

    #[test]
    fn migrate_agent_invocation_fn_components() {
        let mut migrate_agent_invocation =
            MigrateAgentInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();

        migrate_agent_invocation.migrate_agent = MigrateAgent::Open;

        let mut expected = vec!["migrate_agent", "migrate_agent_open"];
        for fn_component in migrate_agent_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn migrate_agent_invocation_host_input() {
        let migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let host_input = migrate_agent_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(MigrateAgentFixturator::new(::fixt::Empty).next().unwrap()).unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::MigrateAgentResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::MigrateAgentHostAccessFixturator;
    use crate::fixt::MigrateAgentInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn test_migrate_agent_unimplemented() {
        let host_access = MigrateAgentHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let result = ribosome
            .run_migrate_agent(host_access, migrate_agent_invocation)
            .unwrap();
        assert_eq!(result, MigrateAgentResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_migrate_agent_implemented_pass() {
        let host_access = MigrateAgentHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::MigrateAgentPass]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let result = ribosome
            .run_migrate_agent(host_access, migrate_agent_invocation)
            .unwrap();
        assert_eq!(result, MigrateAgentResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_migrate_agent_implemented_fail() {
        let host_access = MigrateAgentHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::MigrateAgentFail]))
            .next()
            .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let result = ribosome
            .run_migrate_agent(host_access, migrate_agent_invocation)
            .unwrap();
        assert_eq!(
            result,
            MigrateAgentResult::Fail(TestWasm::MigrateAgentFail.into(), "no migrate".into()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_migrate_agent_multi_implemented_fail() {
        let host_access = MigrateAgentHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![
            TestWasm::MigrateAgentPass,
            TestWasm::MigrateAgentFail,
        ]))
        .next()
        .unwrap();
        let mut migrate_agent_invocation = MigrateAgentInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        migrate_agent_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let result = ribosome
            .run_migrate_agent(host_access, migrate_agent_invocation)
            .unwrap();
        assert_eq!(
            result,
            MigrateAgentResult::Fail(TestWasm::MigrateAgentFail.into(), "no migrate".into()),
        );
    }
}
