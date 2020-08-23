use holochain_types::dna::wasm::DnaWasm;
pub extern crate strum;
#[macro_use]
extern crate strum_macros;
use holochain_types::dna::zome::Zome;
use holochain_zome_types::zome::ZomeName;

const WASM_WORKSPACE_TARGET: &'static str = "wasm_workspace/target";

#[derive(EnumIter, Clone, Copy)]
pub enum TestWasm {
    AgentInfo,
    Anchor,
    Bench,
    CommitEntry,
    Crud,
    Debug,
    EntryDefs,
    EntryHash,
    Foo,
    HashPath,
    Imports,
    InitFail,
    InitPass,
    Link,
    MigrateAgentFail,
    MigrateAgentPass,
    PostCommitFail,
    PostCommitSuccess,
    SerRegression,
    Validate,
    ValidateLink,
    ValidateInvalid,
    ValidateLinkAddInvalid,
    ValidateValid,
    ValidateLinkAddValid,
    ValidationPackageFail,
    ValidationPackageSuccess,
    WhoAmI,
}

impl From<TestWasm> for ZomeName {
    fn from(test_wasm: TestWasm) -> ZomeName {
        ZomeName::from(match test_wasm {
            TestWasm::AgentInfo => "agent_info",
            TestWasm::Anchor => "anchor",
            TestWasm::Bench => "bench",
            TestWasm::CommitEntry => "commit_entry",
            TestWasm::Crud => "crud",
            TestWasm::Debug => "debug",
            TestWasm::EntryDefs => "entry_defs",
            TestWasm::EntryHash => "entry_hash",
            TestWasm::Foo => "foo",
            TestWasm::HashPath => "hash_path",
            TestWasm::Imports => "imports",
            TestWasm::InitFail => "init_fail",
            TestWasm::InitPass => "init_pass",
            TestWasm::Link => "link",
            TestWasm::MigrateAgentFail => "migrate_agent_fail",
            TestWasm::MigrateAgentPass => "migrate_agent_pass",
            TestWasm::PostCommitFail => "post_commit_fail",
            TestWasm::PostCommitSuccess => "post_commit_success",
            TestWasm::SerRegression => "ser_regression",
            TestWasm::Validate => "validate",
            TestWasm::ValidateLink => "validate_link",
            TestWasm::ValidateInvalid => "validate_invalid",
            TestWasm::ValidateLinkAddInvalid => "validate_link_add_invalid",
            TestWasm::ValidateValid => "validate_valid",
            TestWasm::ValidateLinkAddValid => "validate_link_add_valid",
            TestWasm::ValidationPackageFail => "validation_package_fail",
            TestWasm::ValidationPackageSuccess => "validation_package_success",
            TestWasm::WhoAmI => "whoami",
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
            TestWasm::CommitEntry => {
                get_code("wasm32-unknown-unknown/release/test_wasm_commit_entry.wasm")
            }
            TestWasm::Crud => get_code("wasm32-unknown-unknown/release/test_wasm_crud.wasm"),
            TestWasm::Debug => get_code("wasm32-unknown-unknown/release/test_wasm_debug.wasm"),
            TestWasm::EntryDefs => {
                get_code("wasm32-unknown-unknown/release/test_wasm_entry_defs.wasm")
            }
            TestWasm::EntryHash => {
                get_code("wasm32-unknown-unknown/release/test_wasm_entry_hash.wasm")
            }
            TestWasm::Foo => get_code("wasm32-unknown-unknown/release/test_wasm_foo.wasm"),
            TestWasm::HashPath => {
                get_code("wasm32-unknown-unknown/release/test_wasm_hash_path.wasm")
            }
            TestWasm::Imports => get_code("wasm32-unknown-unknown/release/test_wasm_imports.wasm"),
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
            TestWasm::PostCommitFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_post_commit_fail.wasm")
            }
            TestWasm::PostCommitSuccess => {
                get_code("wasm32-unknown-unknown/release/test_wasm_post_commit_success.wasm")
            }
            TestWasm::SerRegression => {
                get_code("wasm32-unknown-unknown/release/test_wasm_ser_regression.wasm")
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
            TestWasm::ValidateLinkAddInvalid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_link_add_invalid.wasm")
            }
            TestWasm::ValidateValid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_valid.wasm")
            }
            TestWasm::ValidateLinkAddValid => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validate_link_add_valid.wasm")
            }
            TestWasm::ValidationPackageFail => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validation_package_fail.wasm")
            }
            TestWasm::ValidationPackageSuccess => {
                get_code("wasm32-unknown-unknown/release/test_wasm_validation_package_success.wasm")
            }
            TestWasm::WhoAmI => get_code("wasm32-unknown-unknown/release/test_wasm_whoami.wasm"),
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

impl From<TestWasm> for Zome {
    fn from(test_wasm: TestWasm) -> Self {
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            let dna_wasm: DnaWasm = test_wasm.into();
            let (_, wasm_hash) = holochain_types::dna::wasm::DnaWasmHashed::from_content(dna_wasm)
                .await
                .into_inner();
            Self { wasm_hash }
        })
    }
}

impl From<TestWasm> for (ZomeName, Zome) {
    fn from(test_wasm: TestWasm) -> Self {
        (test_wasm.into(), test_wasm.into())
    }
}
