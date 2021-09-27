use holochain_types::prelude::*;
use strum_macros::EnumIter;

const WASM_WORKSPACE_TARGET: &str = "wasm_workspace/target";

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
    PostCommitFail,
    PostCommitSuccess,
    Query,
    RandomBytes,
    Schedule,
    XSalsa20Poly1305,
    SerRegression,
    Sign,
    SysTime,
    Update,
    Validate,
    ValidateLink,
    ValidateInvalid,
    ValidateCreateLinkInvalid,
    ValidateValid,
    ValidateCreateLinkValid,
    ValidationPackageFail,
    ValidationPackageSuccess,
    WhoAmI,
    ZomeInfo,
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
            TestWasm::PostCommitFail => "post_commit_fail",
            TestWasm::PostCommitSuccess => "post_commit_success",
            TestWasm::Query => "query",
            TestWasm::RandomBytes => "random_bytes",
            TestWasm::Schedule => "schedule",
            TestWasm::XSalsa20Poly1305 => "x_salsa20_poly1305",
            TestWasm::SerRegression => "ser_regression",
            TestWasm::Sign => "sign",
            TestWasm::SysTime => "sys_time",
            TestWasm::Update => "update_entry",
            TestWasm::Validate => "validate",
            TestWasm::ValidateLink => "validate_link",
            TestWasm::ValidateInvalid => "validate_invalid",
            TestWasm::ValidateCreateLinkInvalid => "validate_link_add_invalid",
            TestWasm::ValidateValid => "validate_valid",
            TestWasm::ValidateCreateLinkValid => "validate_link_add_valid",
            TestWasm::ValidationPackageFail => "validation_package_fail",
            TestWasm::ValidationPackageSuccess => "validation_package_success",
            TestWasm::WhoAmI => "whoami",
            TestWasm::ZomeInfo => "zome_info",
        })
    }
}

impl From<TestWasm> for DnaWasm {
    fn from(test_wasm: TestWasm) -> DnaWasm {
        DnaWasm::from(match test_wasm {
            TestWasm::AgentInfo => {
                get_code("wasm32-unknown-unknown/release/test_wasm_agent_info.wasm")
            }
            TestWasm::Anchor => get_code("wasm32-unknown-unknown/release/test_wasm_anchor.wasm"),
            TestWasm::Bench => get_code("wasm32-unknown-unknown/release/test_wasm_bench.wasm"),
            TestWasm::Capability => {
                get_code("wasm32-unknown-unknown/release/test_wasm_capability.wasm")
            }
            TestWasm::CounterSigning => {
                get_code("wasm32-unknown-unknown/release/test_wasm_countersigning.wasm")
            }
            TestWasm::Create => {
                get_code("wasm32-unknown-unknown/release/test_wasm_create_entry.wasm")
            }
            TestWasm::Crd => get_code("wasm32-unknown-unknown/release/test_wasm_crd.wasm"),
            TestWasm::Crud => get_code("wasm32-unknown-unknown/release/test_wasm_crud.wasm"),
            TestWasm::Debug => get_code("wasm32-unknown-unknown/release/test_wasm_debug.wasm"),
            TestWasm::EntryDefs => {
                get_code("wasm32-unknown-unknown/release/test_wasm_entry_defs.wasm")
            }
            TestWasm::EmitSignal => {
                get_code("wasm32-unknown-unknown/release/test_wasm_emit_signal.wasm")
            }
            TestWasm::HashEntry => {
                get_code("wasm32-unknown-unknown/release/test_wasm_hash_entry.wasm")
            }
            TestWasm::Foo => get_code("wasm32-unknown-unknown/release/test_wasm_foo.wasm"),
            TestWasm::GenesisSelfCheckInvalid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_genesis_self_check_invalid.wasm")
            }
            TestWasm::GenesisSelfCheckValid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_genesis_self_check_valid.wasm")
            }
            TestWasm::HashPath => {
                get_code("wasm32-unknown-unknown/release/test_wasm_hash_path.wasm")
            }
            TestWasm::HdkExtern => {
                get_code("wasm32-unknown-unknown/release/test_wasm_hdk_extern.wasm")
            }
            TestWasm::InitFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_init_fail.wasm")
            }
            TestWasm::InitPass => {
                get_code("wasm32-unknown-unknown/release/test_wasm_init_pass.wasm")
            }
            TestWasm::Link => get_code("wasm32-unknown-unknown/release/test_wasm_link.wasm"),
            TestWasm::MigrateAgentFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_migrate_agent_fail.wasm")
            }
            TestWasm::MigrateAgentPass => {
                get_code("wasm32-unknown-unknown/release/test_wasm_migrate_agent_pass.wasm")
            }
            TestWasm::MultipleCalls => {
                get_code("wasm32-unknown-unknown/release/test_wasm_multiple_calls.wasm")
            }
            TestWasm::MustGet => get_code("wasm32-unknown-unknown/release/test_wasm_must_get.wasm"),
            TestWasm::PostCommitFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_post_commit_fail.wasm")
            }
            TestWasm::PostCommitSuccess => {
                get_code("wasm32-unknown-unknown/release/test_wasm_post_commit_success.wasm")
            }
            TestWasm::Query => get_code("wasm32-unknown-unknown/release/test_wasm_query.wasm"),
            TestWasm::RandomBytes => {
                get_code("wasm32-unknown-unknown/release/test_wasm_random_bytes.wasm")
            }
            TestWasm::Schedule => {
                get_code("wasm32-unknown-unknown/release/test_wasm_schedule.wasm")
            }
            TestWasm::XSalsa20Poly1305 => {
                get_code("wasm32-unknown-unknown/release/test_wasm_x_salsa20_poly1305.wasm")
            }
            TestWasm::SerRegression => {
                get_code("wasm32-unknown-unknown/release/test_wasm_ser_regression.wasm")
            }
            TestWasm::Sign => get_code("wasm32-unknown-unknown/release/test_wasm_sign.wasm"),
            TestWasm::SysTime => get_code("wasm32-unknown-unknown/release/test_wasm_sys_time.wasm"),
            TestWasm::Update => {
                get_code("wasm32-unknown-unknown/release/test_wasm_update_entry.wasm")
            }
            TestWasm::Validate => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate.wasm")
            }
            TestWasm::ValidateLink => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_link.wasm")
            }
            TestWasm::ValidateInvalid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_invalid.wasm")
            }
            TestWasm::ValidateCreateLinkInvalid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_link_add_invalid.wasm")
            }
            TestWasm::ValidateValid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_valid.wasm")
            }
            TestWasm::ValidateCreateLinkValid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_link_add_valid.wasm")
            }
            TestWasm::ValidationPackageFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validation_package_fail.wasm")
            }
            TestWasm::ValidationPackageSuccess => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validation_package_success.wasm")
            }
            TestWasm::WhoAmI => get_code("wasm32-unknown-unknown/release/test_wasm_whoami.wasm"),
            TestWasm::ZomeInfo => {
                get_code("wasm32-unknown-unknown/release/test_wasm_zome_info.wasm")
            }
        })
    }
}

fn get_code(path: &'static str) -> Vec<u8> {
    let path = match option_env!("HC_TEST_WASM_DIR") {
        Some(dir) => format!("{}/{}", dir, path),
        None => format!(
            "{}/{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            WASM_WORKSPACE_TARGET,
            path
        ),
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

impl From<TestWasm> for ZomeDef {
    fn from(test_wasm: TestWasm) -> Self {
        tokio_helper::block_forever_on(async move {
            let dna_wasm: DnaWasm = test_wasm.into();
            let (_, wasm_hash) = holochain_types::dna::wasm::DnaWasmHashed::from_content(dna_wasm)
                .await
                .into_inner();
            ZomeDef::Wasm(WasmZome { wasm_hash })
        })
    }
}

impl From<TestWasm> for (ZomeName, ZomeDef) {
    fn from(test_wasm: TestWasm) -> Self {
        (test_wasm.into(), test_wasm.into())
    }
}

impl From<TestWasm> for Zome {
    fn from(test_wasm: TestWasm) -> Self {
        Zome::new(test_wasm.into(), test_wasm.into())
    }
}
