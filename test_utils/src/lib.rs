#![warn(unused_extern_crates)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate holochain_json_derive;

pub mod mock_signing;

use crossbeam_channel::Receiver;
use holochain_conductor_lib::{context_builder::ContextBuilder, error::HolochainResult, Holochain};
use holochain_core::{
    action::Action,
    context::Context,
    logger::{test_logger, TestLogger},
    nucleus::actions::call_zome_function::make_cap_request_for_call,
    signal::{signal_channel, Signal, SignalReceiver},
};
use holochain_core_types::{
    crud_status::CrudStatus,
    dna::{
        entry_types::{EntryTypeDef, LinkedFrom, LinksTo, Sharing},
        fn_declarations::{FnDeclaration, TraitFns},
        traits::ReservedTraitNames,
        wasm::DnaWasm,
        zome::{Config, Zome, ZomeFnDeclarations, ZomeTraits},
        Dna,
    },
    entry::{
        entry_type::{test_app_entry_type, AppEntryType, EntryType},
        Entry, EntryWithMeta,
    },
};
use holochain_json_api::{error::JsonError, json::JsonString};
use holochain_locksmith::Mutex;
use holochain_persistence_api::cas::content::{Address, AddressableContent};

use holochain_net::p2p_config::P2pConfig;

use holochain_wasm_utils::{
    api_serialization::get_entry::{GetEntryResult, StatusRequestKind},
    wasm_target_dir,
};

use hdk::error::{ZomeApiError, ZomeApiResult};

use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    env,
    fs::File,
    hash::{Hash, Hasher},
    io::prelude::*,
    path::PathBuf,
    sync::Arc,
    thread,
    time::Duration,
};
use tempfile::tempdir;
use wabt::Wat2Wasm;

lazy_static! {
    pub static ref DYNAMO_DB_LOCAL_TEST_HOST_PATH: &'static str = "http://localhost:8001";
}

/// Load WASM from filesystem
pub fn create_wasm_from_file(path: &PathBuf) -> Vec<u8> {
    let mut file = File::open(path)
        .unwrap_or_else(|err| panic!("Couldn't create WASM from file: {:?}; {}", path, err));
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

/// Create DNA from WAT
pub fn create_test_dna_with_wat(zome_name: &str, wat: Option<&str>) -> Dna {
    // Default WASM code returns 1337 as integer
    let default_wat = r#"
            (module
                (memory (;0;) 1)
                (func (export "public_test_fn") (param $p0 i64) (result i64)
                    i64.const 6
                )
                (data (i32.const 0)
                    "1337.0"
                )
                (export "memory" (memory 0))
            )
        "#;
    let wat_str = wat.unwrap_or_else(|| &default_wat);

    // Test WASM code that returns 1337 as integer
    let wasm_binary = Wat2Wasm::new()
        .canonicalize_lebs(false)
        .write_debug_names(true)
        .convert(wat_str)
        .unwrap();

    create_test_dna_with_wasm(zome_name, wasm_binary.as_ref().to_vec())
}

/// Prepare valid DNA struct with that WASM in a zome's capability
pub fn create_test_dna_with_wasm(zome_name: &str, wasm: Vec<u8>) -> Dna {
    let mut dna = Dna::new();
    let defs = create_test_defs_with_fn_name("public_test_fn");

    //    let mut capabilities = BTreeMap::new();
    //    capabilities.insert(cap_name.to_string(), capability);

    let mut test_entry_def = EntryTypeDef::new();
    test_entry_def.links_to.push(LinksTo {
        target_type: String::from("testEntryType"),
        link_type: String::from("test-link"),
    });

    let mut test_entry_b_def = EntryTypeDef::new();
    test_entry_b_def.linked_from.push(LinkedFrom {
        base_type: String::from("testEntryType"),
        link_type: String::from("test-link"),
    });

    let mut test_entry_c_def = EntryTypeDef::new();
    test_entry_c_def.sharing = Sharing::Private;

    let mut entry_types = BTreeMap::new();

    entry_types.insert(
        EntryType::App(AppEntryType::from("testEntryType")),
        test_entry_def,
    );
    entry_types.insert(
        EntryType::App(AppEntryType::from("testEntryTypeB")),
        test_entry_b_def,
    );
    entry_types.insert(
        EntryType::App(AppEntryType::from("testEntryTypeC")),
        test_entry_c_def,
    );

    let mut zome = Zome::new(
        "some zome description",
        &Config::new(),
        &entry_types,
        &defs.0,
        &defs.1,
        &DnaWasm::from_bytes(wasm),
    );

    let mut trait_fns = TraitFns::new();
    trait_fns.functions.push("public_test_fn".to_string());
    zome.traits
        .insert(ReservedTraitNames::Public.as_str().to_string(), trait_fns);
    dna.zomes.insert(zome_name.to_string(), zome);
    dna.name = "TestApp".into();
    dna.uuid = "8ed84a02-a0e6-4c8c-a752-34828e302986".into();
    dna
}

pub fn create_test_defs_with_fn_name(fn_name: &str) -> (ZomeFnDeclarations, ZomeTraits) {
    let mut trait_fns = TraitFns::new();
    let mut fn_decl = FnDeclaration::new();
    fn_decl.name = String::from(fn_name);
    trait_fns.functions.push(String::from(fn_name));
    let mut traits = BTreeMap::new();
    traits.insert(ReservedTraitNames::Public.as_str().to_string(), trait_fns);

    let mut functions = Vec::new();
    functions.push(fn_decl);
    (functions, traits)
}

pub fn create_test_defs_with_fn_names(fn_names: Vec<String>) -> (ZomeFnDeclarations, ZomeTraits) {
    let mut trait_fns = TraitFns::new();
    let mut functions = Vec::new();
    for fn_name in fn_names {
        let mut fn_decl = FnDeclaration::new();
        fn_decl.name = fn_name.clone();
        functions.push(fn_decl);
        trait_fns.functions.push(fn_name.clone());
    }

    let mut traits = BTreeMap::new();
    traits.insert(ReservedTraitNames::Public.as_str().to_string(), trait_fns);

    (functions, traits)
}

pub fn create_test_defs_with_hc_public_fn_names(
    fn_names: Vec<&str>,
) -> (ZomeFnDeclarations, ZomeTraits) {
    let mut traitfns = TraitFns::new();
    let mut fn_declarations = Vec::new();

    for fn_name in fn_names {
        traitfns.functions.push(String::from(fn_name));
        let mut fn_decl = FnDeclaration::new();
        fn_decl.name = String::from(fn_name);
        fn_declarations.push(fn_decl);
    }
    let mut traits = BTreeMap::new();
    traits.insert("hc_public".to_string(), traitfns);
    (fn_declarations, traits)
}

/// Prepare valid DNA struct with that WASM in a zome's capability
pub fn create_test_dna_with_defs(
    zome_name: &str,
    defs: (ZomeFnDeclarations, ZomeTraits),
    wasm: &[u8],
) -> Dna {
    let mut dna = Dna::new();
    let etypedef = EntryTypeDef::new();
    let mut entry_types = BTreeMap::new();
    entry_types.insert("testEntryType".into(), etypedef);
    let zome = Zome::new(
        "some zome description",
        &Config::new(),
        &entry_types,
        &defs.0,
        &defs.1,
        &DnaWasm::from_bytes(wasm.to_owned()),
    );

    dna.zomes.insert(zome_name.to_string(), zome);
    dna.name = "TestApp".into();
    dna.uuid = "8ed84a02-a0e6-4c8c-a752-34828e302986".into();
    dna
}

pub fn create_arbitrary_test_dna() -> Dna {
    let wat = r#"
    (module
     (memory 1)
     (export "memory" (memory 0))
     (export "public_test_fn" (func $func0))
     (func $func0 (param $p0 i64) (result i64)
           i64.const 16
           )
     (data (i32.const 0)
           "{\"holo\":\"world\"}"
           )
     )
    "#;
    create_test_dna_with_wat("test_zome", Some(wat))
}

#[derive(Clone, Deserialize)]
pub enum TestNodeConfig {
    MemoryGhostEngine(Vec<url::Url>),
    Sim1h(&'static str),
    LegacyInMemory,
}

#[cfg_attr(tarpaulin, skip)]
pub fn create_test_context_with_logger_and_signal(
    agent_name: &str,
    network_name: Option<&str>,
    test_config: TestNodeConfig,
) -> (Arc<Context>, Arc<Mutex<TestLogger>>, SignalReceiver) {
    let agent = mock_signing::registered_test_agent(agent_name);
    let (signal, recieve) = signal_channel();
    let logger = test_logger();
    (
        Arc::new({
            let mut builder = ContextBuilder::new()
                .with_agent(agent.clone())
                .with_file_storage(tempdir().unwrap().path().to_str().unwrap())
                .expect("Tempdir must be accessible")
                .with_conductor_api(mock_signing::mock_conductor_api(agent))
                .with_signals(signal);
            if let Some(network_name) = network_name {
                let config = match test_config {
                    TestNodeConfig::Sim1h(dynamo_db_path) => {
                        P2pConfig::new_with_sim1h_backend(&dynamo_db_path)
                    }
                    TestNodeConfig::MemoryGhostEngine(boostrap_nodes) => {
                        P2pConfig::new_with_memory_lib3h_backend(network_name, boostrap_nodes)
                    }
                    TestNodeConfig::LegacyInMemory => {
                        P2pConfig::new_with_memory_backend(network_name)
                    }
                };
                builder = builder.with_p2p_config(config);
            }
            builder.with_instance_name("test_context_instance").spawn()
        }),
        logger,
        recieve,
    )
}

/// calculates the native Rust hash
/// has nothing to do with our hashing e.g. multihash
/// @see https://doc.rust-lang.org/std/hash/index.html
pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

// Function called at start of all unit tests:
//   Startup holochain and do a call on the specified wasm function.
pub fn hc_setup_and_call_zome_fn<J: Into<JsonString>>(
    wasm_path: &PathBuf,
    fn_name: &str,
    params: J,
) -> HolochainResult<JsonString> {
    // Setup the holochain instance
    let wasm = create_wasm_from_file(wasm_path);
    let defs = create_test_defs_with_fn_name(fn_name);
    let dna = create_test_dna_with_defs("test_zome", defs, &wasm);

    let context = create_test_context("alex");
    let mut hc = Holochain::new(dna.clone(), context.clone()).unwrap();

    let params_string = String::from(params.into());
    let cap_request = make_cap_request_for_call(
        context.clone(),
        context.clone().agent_id.address(),
        fn_name,
        JsonString::from_json(&params_string.clone()),
    );

    // Run the holochain instance
    hc.start().expect("couldn't start");
    // Call the exposed wasm function
    Holochain::call_zome_function(
        hc.context().unwrap(),
        "test_zome",
        cap_request,
        fn_name,
        &params_string
    )
}

/// create a test context and TestLogger pair so we can use the logger in assertions
pub fn create_test_context(agent_name: &str) -> Arc<Context> {
    let agent = mock_signing::registered_test_agent(agent_name);
    Arc::new(
        ContextBuilder::new()
            .with_agent(agent.clone())
            .with_file_storage(tempdir().unwrap().path().to_str().unwrap())
            .expect("Tempdir must be accessible")
            .with_conductor_api(mock_signing::mock_conductor_api(agent))
            .with_instance_name("fake_instance_name")
            .spawn(),
    )
}

// @TODO this is a first attempt at replacing history.len() tests
// @see https://github.com/holochain/holochain-rust/issues/195
pub fn expect_action<F>(rx: &Receiver<Signal>, f: F) -> Result<Action, String>
where
    F: Fn(&Action) -> bool,
{
    let timeout = 1000;
    loop {
        match rx
            .recv_timeout(Duration::from_millis(timeout))
            .map_err(|e| e.to_string())?
        {
            Signal::Trace(aw) => {
                let action = aw.action().clone();
                if f(&action) {
                    return Ok(action);
                }
            }
            _ => continue,
        }
    }
}

pub fn start_holochain_instance<T: Into<String>>(
    uuid: T,
    agent_name: T,
) -> (Holochain, Arc<Mutex<TestLogger>>, SignalReceiver) {
    // Setup the holochain instance

    let mut wasm_path = PathBuf::new();
    let wasm_dir_component: PathBuf = wasm_target_dir(
        &String::from("hdk").into(),
        &String::from("wasm-test").into(),
    );
    wasm_path.push(wasm_dir_component);
    let wasm_path_component: PathBuf = [
        String::from("wasm32-unknown-unknown"),
        String::from("release"),
        String::from("test_globals.wasm"),
    ]
    .iter()
    .collect();
    wasm_path.push(wasm_path_component);

    let wasm = create_wasm_from_file(&wasm_path);

    let defs = create_test_defs_with_hc_public_fn_names(vec![
        "check_global",
        "check_commit_entry",
        "check_commit_entry_macro",
        "check_get_entry_result",
        "check_get_entry",
        "send_tweet",
        "commit_validation_package_tester",
        "link_two_entries",
        "links_roundtrip_create",
        "links_roundtrip_get",
        "links_roundtrip_get_and_load",
        "link_validation",
        "check_query",
        "check_app_entry_address",
        "check_sys_entry_address",
        "check_call",
        "check_call_with_args",
        "send_message",
        "sleep",
        "remove_link",
        "get_entry_properties",
        "emit_signal",
        "show_env",
        "hash_entry",
        "sign_message",
        "verify_message",
        "add_seed",
        "add_key",
        "get_pubkey",
        "list_secrets",
        "create_and_link_tagged_entry",
        "get_my_entries_by_tag",
        "my_entries_with_load",
        "delete_link_tagged_entry",
        "my_entries_immediate_timeout",
        "create_and_link_tagged_entry_bad_link",
        "link_tag_validation",
        "get_entry",
        "create_priv_entry",
        "get_version",
        "sign_payload",
    ]);
    let mut dna = create_test_dna_with_defs("test_zome", defs, &wasm);
    dna.uuid = uuid.into();

    // TODO: construct test DNA using the auto-generated JSON feature
    // The code below is fragile!
    // We have to manually construct a Dna struct that reflects what we defined using define_zome!
    // in wasm-test/src/lib.rs.
    // In a production setting, hc would read the auto-generated JSON to make sure the Dna struct
    // matches up. We should do the same in test.
    {
        let entry_types = &mut dna.zomes.get_mut("test_zome").unwrap().entry_types;
        entry_types.insert(
            EntryType::from("validation_package_tester"),
            EntryTypeDef::new(),
        );
        entry_types.insert(
            EntryType::from("empty_validation_response_tester"),
            EntryTypeDef::new(),
        );
        entry_types.insert(EntryType::from("private test entry"), EntryTypeDef::new());
        let test_entry_type = &mut entry_types
            .get_mut(&EntryType::from("testEntryType"))
            .unwrap();
        test_entry_type.links_to.push(LinksTo {
            target_type: String::from("testEntryType"),
            link_type: String::from("test"),
        });

        test_entry_type.links_to.push(LinksTo {
            target_type: String::from("testEntryType"),
            link_type: String::from("intergration test"),
        });
    }

    {
        let entry_types = &mut dna.zomes.get_mut("test_zome").unwrap().entry_types;
        let mut link_validator = EntryTypeDef::new();
        link_validator.links_to.push(LinksTo {
            target_type: String::from("link_validator"),
            link_type: String::from("longer"),
        });
        entry_types.insert(EntryType::from("link_validator"), link_validator);
    }

    //set this environmental variable to set up the backend for running tests.
    //if none has been set it will default to the legacy in memory worker implementation
    let test_config = env::var("INTEGRATION_TEST_CONFIG")
        .map(|test_config| {
            if test_config == "lib3h" {
                TestNodeConfig::MemoryGhostEngine(vec![])
            } else if test_config == "sim1h" {
                TestNodeConfig::Sim1h(&DYNAMO_DB_LOCAL_TEST_HOST_PATH)
            } else {
                TestNodeConfig::LegacyInMemory
            }
        })
        .unwrap_or(TestNodeConfig::LegacyInMemory);
    let (context, test_logger, signal_recieve) = create_test_context_with_logger_and_signal(
        &dna.uuid,
        Some(&agent_name.into()),
        test_config,
    );
    let mut hc =
        Holochain::new(dna.clone(), context).expect("could not create new Holochain instance.");

    // Run the holochain instance
    hc.start().expect("couldn't start");
    (hc, test_logger, signal_recieve)
}

pub fn make_test_call(
    hc: &mut Holochain,
    fn_name: &str,
    params: &str,
) -> HolochainResult<JsonString> {
    let cap_call = {
        let context = hc.context()?;
        let token = context.get_public_token().unwrap();
        make_cap_request_for_call(
            context.clone(),
            token,
            fn_name,
            JsonString::from_json(params),
        )
    };
    Holochain::call_zome_function(
        hc.context().unwrap(),
        "test_zome",
        cap_call,
        fn_name,
        params
    )
}

#[derive(Deserialize, Serialize, Default, Debug, DefaultJson, Clone)]
pub struct TestEntry {
    pub stuff: String,
}

pub fn example_valid_entry() -> Entry {
    Entry::App(
        test_app_entry_type().into(),
        TestEntry {
            stuff: "non fail".into(),
        }
        .into(),
    )
}

pub fn empty_string_validation_fail_entry() -> Entry {
    Entry::App(
        "empty_validation_response_tester".into(),
        TestEntry {
            stuff: "should fail with empty string".into(),
        }
        .into(),
    )
}

pub fn example_valid_entry_result() -> GetEntryResult {
    let entry = example_valid_entry();
    let entry_with_meta = &EntryWithMeta {
        entry: entry.clone(),
        crud_status: CrudStatus::Live,
        maybe_link_update_delete: None,
    };
    GetEntryResult::new(StatusRequestKind::Latest, Some((entry_with_meta, vec![])))
}

pub fn example_valid_entry_params() -> String {
    format!(
        "{{\"entry\":{}}}",
        String::from(JsonString::from(example_valid_entry())),
    )
}

pub fn example_valid_entry_address() -> Address {
    Address::from("QmefcRdCAXM2kbgLW2pMzqWhUvKSDvwfFSVkvmwKvBQBHd")
}

//this polls for the zome result until it satisfies a the boolean condition or elapses a number of tries.
//only use this for get requests please
pub fn wait_for_zome_result<'a, T>(
    holochain: &mut Holochain,
    zome_call: &str,
    params: &str,
    boolean_condition: fn(T) -> bool,
    tries: i8,
) -> ZomeApiResult<T>
where
    T: hdk::serde::de::DeserializeOwned + Clone,
{
    //make zome call
    let result = make_test_call(holochain, zome_call, params);
    let call_result = result
        .clone()
        .expect("Could not wait for condition as result is malformed")
        .to_string();

    //serialize into ZomeApiResult type
    let expected_result: ZomeApiResult<T> = serde_json::from_str::<ZomeApiResult<T>>(&call_result)
        .map_err(|_| {
            ZomeApiError::Internal(format!("Error converting serde result for {}", zome_call))
        })?;
    let value = expected_result.clone()?;

    //check if condition is satisifed
    if !boolean_condition(value) && tries > 0 {
        thread::sleep(Duration::from_secs(10));

        //recursively call function again and decrement tries so far
        wait_for_zome_result(holochain, zome_call, params, boolean_condition, tries - 1)
    } else {
        expected_result
    }
}

pub fn generate_zome_internal_error(error_kind: String) -> ZomeApiError {
    let path = PathBuf::new()
        .join("crates")
        .join("core")
        .join("src")
        .join("wasm_engine")
        .join("runtime.rs");
    let path_string = path
        .as_path()
        .to_str()
        .expect("path should have been created");
    let formatted_path_string = path_string.replace("\\", &vec!["\\", "\\"].join(""));
    let error_string = format!(
        r#"{{"kind":{},"file":"{}","line":"225"}}"#,
        error_kind, formatted_path_string
    );
    ZomeApiError::Internal(error_string)
}

/// Check that internal errors are equivalent, not including line number,
/// which is fragile
pub fn assert_zome_internal_errors_equivalent(left: &ZomeApiError, right: &ZomeApiError) {
    match (left, right) {
        (ZomeApiError::Internal(left_str), ZomeApiError::Internal(right_str)) => assert_eq!(
            internal_error_substr(left_str),
            internal_error_substr(right_str)
        ),
        _ => panic!("These are not both ZomeApiError::Internal"),
    }
}

fn internal_error_substr<'a>(error_string: &'a str) -> Option<&'a str> {
    error_string
        .find(r#""line":"#)
        .map(|idx| &error_string[0..idx])
}
