use std::ffi::OsString;
use std::path::PathBuf;

use holochain_types::prelude::*;
use strum_macros::EnumIter;

const WASM_WORKSPACE_TARGET: &str = "wasm_workspace/target";

#[derive(EnumIter, Clone, Copy)]
pub enum TestIntegrityWasm {
    IntegrityZome,
}

#[derive(EnumIter, Clone, Copy)]
pub enum TestCoordinatorWasm {
    CoordinatorZome,
    CoordinatorZomeUpdate,
}

#[derive(EnumIter, Clone, Copy)]
pub enum TestWasm {
    AgentInfo,
    Anchor,
    Bench,
    Capability,
    CounterSigning,
    Create,
    Crd,
    Crud,
    Debug,
    EntryDefs,
    EmitSignal,
    HashEntry,
    Foo,
    GenesisSelfCheckInvalid,
    GenesisSelfCheckValid,
    HashPath,
    HdkExtern,
    InitFail,
    InitPass,
    Link,
    MigrateAgentFail,
    MigrateAgentPass,
    MultipleCalls,
    MustGet,
    PostCommitSuccess,
    PostCommitVolley,
    Query,
    RandomBytes,
    Schedule,
    XSalsa20Poly1305,
    RateLimits,
    SerRegression,
    Sign,
    SysTime,
    TheIncredibleHalt,
    Update,
    Validate,
    ValidateLink,
    ValidateInvalid,
    ValidateCreateLinkInvalid,
    ValidateValid,
    ValidateCreateLinkValid,
    Weigh,
    WhoAmI,
    ZomeInfo,
}
/// Utility type for combining a test wasm's coordinator
/// zome with it's integrity zome.
pub struct TestWasmPair<I, C = I> {
    pub integrity: I,
    pub coordinator: C,
}

pub type TestZomes = TestWasmPair<IntegrityZome, CoordinatorZome>;

impl TestWasm {
    /// Get the [`ZomeName`] for the integrity zome.
    pub fn integrity_zome_name(self) -> ZomeName {
        TestWasmPair::<ZomeName>::from(self).integrity
    }
    /// Get the [`ZomeName`] for the coordinator zome.
    pub fn coordinator_zome_name(self) -> ZomeName {
        TestWasmPair::<ZomeName>::from(self).coordinator
    }
    /// Get the [`Zome`] for the integrity zome.
    pub fn integrity_zome(self) -> Zome {
        TestWasmPair::<IntegrityZome, CoordinatorZome>::from(self)
            .integrity
            .erase_type()
    }
    /// Get the [`Zome`] for the coordinator zome.
    pub fn coordinator_zome(self) -> Zome {
        TestWasmPair::<IntegrityZome, CoordinatorZome>::from(self)
            .coordinator
            .erase_type()
    }
}

impl From<TestIntegrityWasm> for ZomeName {
    fn from(test_wasm: TestIntegrityWasm) -> ZomeName {
        ZomeName::from(match test_wasm {
            TestIntegrityWasm::IntegrityZome => "integrity_zome",
        })
    }
}

impl From<TestCoordinatorWasm> for ZomeName {
    fn from(test_wasm: TestCoordinatorWasm) -> ZomeName {
        ZomeName::from(match test_wasm {
            TestCoordinatorWasm::CoordinatorZome => "coordinator_zome",
            TestCoordinatorWasm::CoordinatorZomeUpdate => "coordinator_zome_update",
        })
    }
}

impl From<TestWasm> for ZomeName {
    fn from(test_wasm: TestWasm) -> ZomeName {
        ZomeName::from(match test_wasm {
            TestWasm::AgentInfo => "agent_info",
            TestWasm::Anchor => "anchor",
            TestWasm::Bench => "bench",
            TestWasm::Capability => "capability",
            TestWasm::CounterSigning => "countersigning",
            TestWasm::Create => "create_entry",
            TestWasm::Crd => "crd",
            TestWasm::Crud => "crud",
            TestWasm::Debug => "debug",
            TestWasm::EntryDefs => "entry_defs",
            TestWasm::EmitSignal => "emit_signal",
            TestWasm::HashEntry => "hash_entry",
            TestWasm::Foo => "foo",
            TestWasm::GenesisSelfCheckInvalid => "genesis_self_check_invalid",
            TestWasm::GenesisSelfCheckValid => "genesis_self_check_valid",
            TestWasm::HashPath => "hash_path",
            TestWasm::HdkExtern => "hdk_extern",
            TestWasm::InitFail => "init_fail",
            TestWasm::InitPass => "init_pass",
            TestWasm::Link => "link",
            TestWasm::MigrateAgentFail => "migrate_agent_fail",
            TestWasm::MigrateAgentPass => "migrate_agent_pass",
            TestWasm::MultipleCalls => "multiple_calls",
            TestWasm::MustGet => "must_get",
            TestWasm::PostCommitSuccess => "post_commit_success",
            TestWasm::PostCommitVolley => "post_commit_volley",
            TestWasm::Query => "query",
            TestWasm::RandomBytes => "random_bytes",
            TestWasm::RateLimits => "rate_limits",
            TestWasm::Schedule => "schedule",
            TestWasm::XSalsa20Poly1305 => "x_salsa20_poly1305",
            TestWasm::SerRegression => "ser_regression",
            TestWasm::Sign => "sign",
            TestWasm::SysTime => "sys_time",
            TestWasm::TheIncredibleHalt => "the_incredible_halt",
            TestWasm::Update => "update_entry",
            TestWasm::Validate => "validate",
            TestWasm::ValidateLink => "validate_link",
            TestWasm::ValidateInvalid => "validate_invalid",
            TestWasm::ValidateCreateLinkInvalid => "validate_link_add_invalid",
            TestWasm::ValidateValid => "validate_valid",
            TestWasm::ValidateCreateLinkValid => "validate_link_add_valid",
            TestWasm::Weigh => "weigh",
            TestWasm::WhoAmI => "whoami",
            TestWasm::ZomeInfo => "zome_info",
        })
    }
}

impl From<TestWasm> for TestWasmPair<ZomeName> {
    fn from(test_wasm: TestWasm) -> Self {
        let coordinator: ZomeName = test_wasm.into();
        let integrity = ZomeName::new(format!("integrity_{}", coordinator));
        TestWasmPair {
            integrity,
            coordinator,
        }
    }
}

impl From<TestWasm> for PathBuf {
    fn from(test_wasm: TestWasm) -> Self {
        PathBuf::from(match test_wasm {
            TestWasm::AgentInfo => "wasm32-unknown-unknown/release/test_wasm_agent_info.wasm",
            TestWasm::Anchor => "wasm32-unknown-unknown/release/test_wasm_anchor.wasm",
            TestWasm::Bench => "wasm32-unknown-unknown/release/test_wasm_bench.wasm",
            TestWasm::Capability => "wasm32-unknown-unknown/release/test_wasm_capability.wasm",
            TestWasm::CounterSigning => {
                "wasm32-unknown-unknown/release/test_wasm_countersigning.wasm"
            }
            TestWasm::Create => "wasm32-unknown-unknown/release/test_wasm_create_entry.wasm",
            TestWasm::Crd => "wasm32-unknown-unknown/release/test_wasm_crd.wasm",
            TestWasm::Crud => "wasm32-unknown-unknown/release/test_wasm_crud.wasm",
            TestWasm::Debug => "wasm32-unknown-unknown/release/test_wasm_debug.wasm",
            TestWasm::EntryDefs => "wasm32-unknown-unknown/release/test_wasm_entry_defs.wasm",
            TestWasm::EmitSignal => "wasm32-unknown-unknown/release/test_wasm_emit_signal.wasm",
            TestWasm::HashEntry => "wasm32-unknown-unknown/release/test_wasm_hash_entry.wasm",
            TestWasm::Foo => "wasm32-unknown-unknown/release/test_wasm_foo.wasm",
            TestWasm::GenesisSelfCheckInvalid => {
                "wasm32-unknown-unknown/release/test_wasm_genesis_self_check_invalid.wasm"
            }
            TestWasm::GenesisSelfCheckValid => {
                "wasm32-unknown-unknown/release/test_wasm_genesis_self_check_valid.wasm"
            }
            TestWasm::HashPath => "wasm32-unknown-unknown/release/test_wasm_hash_path.wasm",
            TestWasm::HdkExtern => "wasm32-unknown-unknown/release/test_wasm_hdk_extern.wasm",
            TestWasm::InitFail => "wasm32-unknown-unknown/release/test_wasm_init_fail.wasm",
            TestWasm::InitPass => "wasm32-unknown-unknown/release/test_wasm_init_pass.wasm",
            TestWasm::Link => "wasm32-unknown-unknown/release/test_wasm_link.wasm",
            TestWasm::MigrateAgentFail => {
                "wasm32-unknown-unknown/release/test_wasm_migrate_agent_fail.wasm"
            }
            TestWasm::MigrateAgentPass => {
                "wasm32-unknown-unknown/release/test_wasm_migrate_agent_pass.wasm"
            }
            TestWasm::MultipleCalls => {
                "wasm32-unknown-unknown/release/test_wasm_multiple_calls.wasm"
            }
            TestWasm::MustGet => "wasm32-unknown-unknown/release/test_wasm_must_get.wasm",
            TestWasm::PostCommitSuccess => {
                "wasm32-unknown-unknown/release/test_wasm_post_commit_success.wasm"
            }
            TestWasm::PostCommitVolley => {
                "wasm32-unknown-unknown/release/test_wasm_post_commit_volley.wasm"
            }
            TestWasm::Query => "wasm32-unknown-unknown/release/test_wasm_query.wasm",
            TestWasm::RandomBytes => "wasm32-unknown-unknown/release/test_wasm_random_bytes.wasm",
            TestWasm::RateLimits => "wasm32-unknown-unknown/release/test_wasm_rate_limits.wasm",
            TestWasm::Schedule => "wasm32-unknown-unknown/release/test_wasm_schedule.wasm",
            TestWasm::XSalsa20Poly1305 => {
                "wasm32-unknown-unknown/release/test_wasm_x_salsa20_poly1305.wasm"
            }
            TestWasm::SerRegression => {
                "wasm32-unknown-unknown/release/test_wasm_ser_regression.wasm"
            }
            TestWasm::Sign => "wasm32-unknown-unknown/release/test_wasm_sign.wasm",
            TestWasm::SysTime => "wasm32-unknown-unknown/release/test_wasm_sys_time.wasm",
            TestWasm::TheIncredibleHalt => {
                "wasm32-unknown-unknown/release/test_wasm_the_incredible_halt.wasm"
            }
            TestWasm::Update => "wasm32-unknown-unknown/release/test_wasm_update_entry.wasm",
            TestWasm::Validate => "wasm32-unknown-unknown/release/test_wasm_validate.wasm",
            TestWasm::ValidateLink => "wasm32-unknown-unknown/release/test_wasm_validate_link.wasm",
            TestWasm::ValidateInvalid => {
                "wasm32-unknown-unknown/release/test_wasm_validate_invalid.wasm"
            }
            TestWasm::ValidateCreateLinkInvalid => {
                "wasm32-unknown-unknown/release/test_wasm_validate_link_add_invalid.wasm"
            }
            TestWasm::ValidateValid => {
                "wasm32-unknown-unknown/release/test_wasm_validate_valid.wasm"
            }
            TestWasm::ValidateCreateLinkValid => {
                "wasm32-unknown-unknown/release/test_wasm_validate_link_add_valid.wasm"
            }
            TestWasm::Weigh => "wasm32-unknown-unknown/release/test_wasm_weigh.wasm",
            TestWasm::WhoAmI => "wasm32-unknown-unknown/release/test_wasm_whoami.wasm",
            TestWasm::ZomeInfo => "wasm32-unknown-unknown/release/test_wasm_zome_info.wasm",
        })
    }
}

impl From<TestWasm> for DnaWasm {
    fn from(t: TestWasm) -> Self {
        DnaWasm::from(get_code(PathBuf::from(t)))
    }
}

impl From<TestWasm> for Vec<DnaWasm> {
    fn from(t: TestWasm) -> Self {
        let TestWasmPair {
            integrity,
            coordinator,
        } = TestWasmPair::<DnaWasm>::from(t);
        vec![integrity, coordinator]
    }
}

impl From<TestIntegrityWasm> for DnaWasm {
    fn from(t: TestIntegrityWasm) -> Self {
        DnaWasm::from(get_code(PathBuf::from(t)))
    }
}

impl From<TestCoordinatorWasm> for DnaWasm {
    fn from(t: TestCoordinatorWasm) -> Self {
        DnaWasm::from(get_code(PathBuf::from(t)))
    }
}

impl From<TestWasm> for TestWasmPair<DnaWasm> {
    fn from(t: TestWasm) -> Self {
        TestWasmPair::<PathBuf>::from(t).into()
    }
}

impl From<TestWasm> for TestWasmPair<PathBuf> {
    fn from(t: TestWasm) -> Self {
        let coordinator = PathBuf::from(t);
        let mut integrity = coordinator.clone();
        let mut integrity_file_name = OsString::new();
        integrity_file_name.push("integrity_");
        integrity_file_name.push(coordinator.file_name().expect("Must have file name"));
        integrity.pop();
        integrity.push("examples");
        integrity.push(integrity_file_name);
        TestWasmPair {
            integrity,
            coordinator,
        }
    }
}

impl From<TestWasmPair<PathBuf>> for TestWasmPair<DnaWasm> {
    fn from(p: TestWasmPair<PathBuf>) -> Self {
        let TestWasmPair {
            integrity,
            coordinator,
        } = p;
        Self {
            integrity: DnaWasm::from(get_code(integrity)),
            coordinator: DnaWasm::from(get_code(coordinator)),
        }
    }
}

impl From<TestIntegrityWasm> for PathBuf {
    fn from(t: TestIntegrityWasm) -> Self {
        PathBuf::from(match t {
            TestIntegrityWasm::IntegrityZome => {
                "wasm32-unknown-unknown/release/test_wasm_integrity_zome.wasm"
            }
        })
    }
}

impl From<TestCoordinatorWasm> for PathBuf {
    fn from(t: TestCoordinatorWasm) -> Self {
        PathBuf::from(match t {
            TestCoordinatorWasm::CoordinatorZome => {
                "wasm32-unknown-unknown/release/test_wasm_coordinator_zome.wasm"
            }
            TestCoordinatorWasm::CoordinatorZomeUpdate => {
                "wasm32-unknown-unknown/release/test_wasm_coordinator_zome_update.wasm"
            }
        })
    }
}

fn get_code(path: PathBuf) -> Vec<u8> {
    let path = match option_env!("HC_TEST_WASM_DIR") {
        Some(dir) => PathBuf::from(dir).join(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(WASM_WORKSPACE_TARGET)
            .join(path),
    };
    let warning = format!(
        "Wasm: {:?} was not found. Maybe you need to build the test wasms\n
        Run `cargo build --features 'build_wasms' --manifest-path=crates/holochain/Cargo.toml`
        or pass the feature flag to `cargo test`
        ",
        path
    );
    std::fs::read(path).expect(&warning)
}

impl From<TestWasm> for TestWasmPair<IntegrityZomeDef, CoordinatorZomeDef> {
    fn from(test_wasm: TestWasm) -> Self {
        let TestWasmPair {
            integrity,
            coordinator,
        } = TestWasmPair::<PathBuf>::from(test_wasm);
        let TestWasmPair {
            integrity: dep_name,
            ..
        } = TestWasmPair::<ZomeName>::from(test_wasm);
        tokio_helper::block_forever_on(async move {
            TestWasmPair {
                integrity: path_to_def(integrity, Default::default()).await.into(),
                coordinator: path_to_def(coordinator, vec![dep_name]).await.into(),
            }
        })
    }
}

impl From<TestWasm> for TestWasmPair<IntegrityZome, CoordinatorZome> {
    fn from(t: TestWasm) -> Self {
        let TestWasmPair {
            integrity: integrity_name,
            coordinator: coordinator_name,
        } = TestWasmPair::<ZomeName>::from(t);
        let TestWasmPair {
            integrity,
            coordinator,
        } = TestWasmPair::<IntegrityZomeDef, CoordinatorZomeDef>::from(t);
        TestWasmPair {
            integrity: IntegrityZome::new(integrity_name, integrity),
            coordinator: CoordinatorZome::new(coordinator_name, coordinator),
        }
    }
}

impl From<TestWasm> for IntegrityZome {
    fn from(test_wasm: TestWasm) -> Self {
        let TestWasmPair { integrity, .. } = TestWasmPair::<PathBuf>::from(test_wasm);

        let def = tokio_helper::block_forever_on(path_to_def(integrity, Default::default()));
        let TestWasmPair {
            integrity: zome_name,
            ..
        } = TestWasmPair::<ZomeName>::from(test_wasm);
        Self::new(zome_name, def.into())
    }
}

impl From<TestIntegrityWasm> for IntegrityZome {
    fn from(t: TestIntegrityWasm) -> Self {
        let def = tokio_helper::block_forever_on(path_to_def(t.into(), Default::default()));
        Self::new(t.into(), def.into())
    }
}

impl From<TestWasm> for CoordinatorZome {
    fn from(test_wasm: TestWasm) -> Self {
        let TestWasmPair { coordinator, .. } = TestWasmPair::<PathBuf>::from(test_wasm);
        let TestWasmPair {
            integrity: dep_name,
            ..
        } = TestWasmPair::<ZomeName>::from(test_wasm);
        let def = tokio_helper::block_forever_on(path_to_def(coordinator, vec![dep_name]));
        Self::new(test_wasm.into(), def.into())
    }
}

impl From<TestCoordinatorWasm> for TestIntegrityWasm {
    fn from(t: TestCoordinatorWasm) -> Self {
        match t {
            TestCoordinatorWasm::CoordinatorZome | TestCoordinatorWasm::CoordinatorZomeUpdate => {
                Self::IntegrityZome
            }
        }
    }
}
impl From<TestCoordinatorWasm> for CoordinatorZome {
    fn from(t: TestCoordinatorWasm) -> Self {
        let dep_name: ZomeName = TestIntegrityWasm::from(t).into();
        let def = tokio_helper::block_forever_on(path_to_def(t.into(), vec![dep_name]));
        Self::new(t.into(), def.into())
    }
}

async fn path_to_def(path: PathBuf, dependencies: Vec<ZomeName>) -> ZomeDef {
    let wasm = DnaWasm::from(get_code(path));
    let wasm_hash = WasmHash::with_data(&wasm).await;
    ZomeDef::Wasm(WasmZome {
        wasm_hash,
        dependencies,
    })
}
