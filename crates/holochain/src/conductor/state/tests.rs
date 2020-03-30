// FIXME: there are a ton of tests here, pulled over from legacy code. They need to be refactored now that legacy Config has been split in two.
use super::*;
use crate::config::{load_configuration, ConductorState, NetworkConfig};
// use crate::test_fixtures::test_dna_loader;

pub fn example_serialized_network_config() -> String {
    unimplemented!()
    // String::from(JsonString::from(P2pConfig::new_with_unique_memory_backend()))
}

#[test]
fn test_agent_load() {
    let toml = r#"
[[agents]]
id = "bob"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "file/to/serialize"

[[agents]]
id="alex"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "another/file"

[[dnas]]
id="dna"
file="file.dna.json"
hash="QmDontCare"
"#;
    let agents = load_configuration::<ConductorState>(toml).unwrap().agents;
    assert_eq!(agents.get(0).expect("expected at least 2 agents").id, "bob");
    assert_eq!(
        agents
            .get(0)
            .expect("expected at least 2 agents")
            .clone()
            .keystore_file,
        "file/to/serialize"
    );
    assert_eq!(
        agents.get(1).expect("expected at least 2 agents").id,
        "alex"
    );
}

#[test]
fn test_dna_load() {
    let toml = r#"
[[agents]]
id="agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "whatever"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"
"#;
    let dnas = load_configuration::<ConductorState>(toml).unwrap().dnas;
    let dna_config = dnas.get(0).expect("expected at least 1 DNA");
    assert_eq!(dna_config.id, "app spec rust");
    assert_eq!(dna_config.file, "app_spec.dna.json");
    assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());
}

#[test]
fn test_load_complete_config() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec websocket interface"
    [interfaces.driver]
    type = "websocket"
    port = 8888
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec http interface"
    [interfaces.driver]
    type = "http"
    port = 4000
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec domainsocket interface"
    [interfaces.driver]
    type = "domainsocket"
    file = "/tmp/holochain.sock"
    [[interfaces.cells]]
    id = "app spec cell"

[network]
type = "sim2h"
todo = "todo"

[metric_publisher]
type = "cloudwatchlogs"
log_stream_name = "2019-11-22_20-53-31.sim2h_public"
log_group_name = "holochain"

"#;

    let config = load_configuration::<ConductorState>(toml).unwrap();

    assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
    let dnas = config.dnas;
    let dna_config = dnas.get(0).expect("expected at least 1 DNA");
    assert_eq!(dna_config.id, "app spec rust");
    assert_eq!(dna_config.file, "app_spec.dna.json");
    assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

    let cells = config.cells;
    let cell_config = cells.get(0).unwrap();
    assert_eq!(cell_config.id, "app spec cell");
    assert_eq!(cell_config.dna, "app spec rust");
    assert_eq!(cell_config.agent, "test agent");
    assert_eq!(config.logger.logger_level, "debug");
    // assert_eq!(format!("{:?}", config.metric_publisher), "Some(CloudWatchLogs(CloudWatchLogsConfig { region: None, log_group_name: Some(\"holochain\"), log_stream_name: Some(\"2019-11-22_20-53-31.sim2h_public\"), assume_role_arn: None }))");
    // assert_eq!(
    //     config.network.unwrap(),
    //     NetworkConfig::N3h(N3hConfig {
    //         bootstrap_nodes: vec![String::from(
    //             "wss://192.168.0.11:64519/?a=hkYW7TrZUS1hy-i374iRu5VbZP1sSw2mLxP4TSe_YI1H2BJM3v_LgAQnpmWA_iR1W5k-8_UoA1BNjzBSUTVNDSIcz9UG0uaM"
    //         )],
    //         n3h_log_level: String::from("d"),
    //         n3h_mode: String::from("REAL"),
    //         n3h_persistence_path: String::from("/Users/cnorris/.holochain/n3h_persistence"),
    //         n3h_ipc_uri: None,
    //         networking_config_file: Some(String::from(
    //             "/Users/cnorris/.holochain/network_config.json"
    //         )),
    //     })
    // );
}

#[test]
fn test_load_complete_config_default_network() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec websocket interface"
    [interfaces.driver]
    type = "websocket"
    port = 8888
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec http interface"
    [interfaces.driver]
    type = "http"
    port = 4000
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec domainsocket interface"
    [interfaces.driver]
    type = "domainsocket"
    file = "/tmp/holochain.sock"
    [[interfaces.cells]]
    id = "app spec cell"

[logger]
type = "debug"
    [[logger.rules.rules]]
    pattern = ".*"
    color = "red"

[[ui_bundles]]
id = "bundle1"
root_dir = "" # serves the current directory
hash = "Qm000"

[[ui_interfaces]]
id = "ui-interface-1"
bundle = "bundle1"
port = 3000
dna_interface = "app spec domainsocket interface"
"#;

    let config = load_configuration::<ConductorState>(toml).unwrap();

    assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
    let dnas = config.dnas;
    let dna_config = dnas.get(0).expect("expected at least 1 DNA");
    assert_eq!(dna_config.id, "app spec rust");
    assert_eq!(dna_config.file, "app_spec.dna.json");
    assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

    let cells = config.cells;
    let cell_config = cells.get(0).unwrap();
    assert_eq!(cell_config.id, "app spec cell");
    assert_eq!(cell_config.dna, "app spec rust");
    assert_eq!(cell_config.agent, "test agent");
    assert_eq!(config.logger.logger_level, "debug");
    // assert_eq!(config.logger.rules.rules.len(), 1);

    assert_eq!(config.network, None);
}

#[test]
fn test_load_bad_network_config() {
    let base_toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec websocket interface"
    [interfaces.driver]
    type = "websocket"
    port = 8888
    [[interfaces.cells]]
    id = "app spec cell"
"#;

    let toml = format!(
        "{}{}",
        base_toml,
        r#"
[network]
type = "lib3h"
"#
    );
    if let Err(e) = load_configuration::<ConductorState>(toml.as_str()) {
        assert!(
            true,
            e.to_string().contains(
                "Error loading configuration: missing field `socket_type` for key `network`"
            )
        )
    } else {
        panic!("Should have failed!")
    }
}

#[test]
fn test_inconsistent_config() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "WRONG DNA ID"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"
"#;

    let config: ConductorState =
        load_configuration(toml).expect("Failed to load config from toml string");

    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err(
            "DNA configuration \"WRONG DNA ID\" not found, mentioned in cell \"app spec cell\""
                .to_string()
                .into()
        )
    );
}

#[test]
fn test_inconsistent_config_interface_1() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec interface"
    [interfaces.driver]
    type = "websocket"
    port = 8888
    [[interfaces.cells]]
    id = "WRONG cell ID"
"#;

    let config = load_configuration::<ConductorState>(toml).unwrap();

    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err(
            "cell configuration \"WRONG cell ID\" not found, mentioned in interface"
                .to_string()
                .into()
        )
    );
}

#[test]
fn test_invalid_toml_1() {
    let toml = &format!(
        r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app-spec-rust.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
network = "{}"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec interface"
    [interfaces.driver]
    type = "invalid type"
    port = 8888
    [[interfaces.cells]]
    id = "app spec cell"
"#,
        example_serialized_network_config()
    );
    if let Err(e) = load_configuration::<ConductorState>(toml) {
        assert!(
            true,
            e.to_string().contains("unknown variant `invalid type`")
        )
    } else {
        panic!("Should have failed!")
    }
}

fn bridges_config(bridges: &str) -> String {
    format!(
        r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "bridge caller"
file = "bridge/caller_without_required.dna"
hash = "Qm328wyq38924y"

[[cells]]
id = "app1"
dna = "bridge caller"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app1_spec_storage"

[[cells]]
id = "app2"
dna = "bridge caller"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app2_spec_storage"

[[cells]]
id = "app3"
dna = "bridge caller"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app3_spec_storage"

{}
"#,
        bridges
    )
}

#[test]
fn test_bridge_config() {
    let toml = bridges_config(
        r#"
[[bridges]]
caller_id = "app1"
callee_id = "app2"
handle = "happ-store"

[[bridges]]
caller_id = "app2"
callee_id = "app3"
handle = "DPKI"
"#,
    );
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));

    // "->": calls
    // app1 -> app2 -> app3
    // app3 has no dependency so it can be instantiated first.
    // app2 depends on (calls) only app3, so app2 is next.
    // app1 should be last.
    assert_eq!(
        config.cell_ids_sorted_by_bridge_dependencies(),
        Ok(vec![
            String::from("app3"),
            String::from("app2"),
            String::from("app1")
        ])
    );
}

#[test]
fn test_bridge_cycle() {
    let toml = bridges_config(
        r#"
[[bridges]]
caller_id = "app1"
callee_id = "app2"
handle = "happ-store"

[[bridges]]
caller_id = "app2"
callee_id = "app3"
handle = "DPKI"

[[bridges]]
caller_id = "app3"
callee_id = "app1"
handle = "test-callee"
"#,
    );
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err("Cyclic dependency in bridge configuration"
            .to_string()
            .into())
    );
}

#[test]
fn test_bridge_non_existent() {
    let toml = bridges_config(
        r#"
[[bridges]]
caller_id = "app1"
callee_id = "app2"
handle = "happ-store"

[[bridges]]
caller_id = "app2"
callee_id = "app3"
handle = "DPKI"

[[bridges]]
caller_id = "app9000"
callee_id = "app1"
handle = "something"
"#,
    );
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err(
            "cell configuration \"app9000\" not found, mentioned in bridge"
                .to_string()
                .into()
        )
    );
}

#[test]
fn test_bridge_dependencies() {
    let toml = bridges_config(
        r#"
[[bridges]]
caller_id = "app1"
callee_id = "app2"
handle = "happ-store"

[[bridges]]
caller_id = "app1"
callee_id = "app3"
handle = "happ-store"

[[bridges]]
caller_id = "app2"
callee_id = "app1"
handle = "happ-store"
"#,
    );
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    let bridged_ids: Vec<_> = config
        .bridge_dependencies(String::from("app1"))
        .iter()
        .map(|bridge| bridge.callee_id.clone())
        .collect();
    assert_eq!(
        bridged_ids,
        vec![String::from("app2"), String::from("app3"),]
    );
}

#[test]
fn test_inconsistent_ui_interface() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "app spec rust"
file = "app_spec.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "app spec cell"
dna = "app spec rust"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "app_spec_storage"

[[interfaces]]
id = "app spec websocket interface"
    [interfaces.driver]
    type = "websocket"
    port = 8888
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec http interface"
    [interfaces.driver]
    type = "http"
    port = 4000
    [[interfaces.cells]]
    id = "app spec cell"

[[interfaces]]
id = "app spec domainsocket interface"
    [interfaces.driver]
    type = "domainsocket"
    file = "/tmp/holochain.sock"
    [[interfaces.cells]]
    id = "app spec cell"

[logger]
type = "debug"
    [[logger.rules.rules]]
    pattern = ".*"
    color = "red"

[[ui_bundles]]
id = "bundle1"
root_dir = "" # serves the current directory
hash = "Qm000"

[[ui_interfaces]]
id = "ui-interface-1"
bundle = "bundle1"
port = 3000
dna_interface = "<not existant>"
"#;
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err("DNA Interface configuration \"<not existant>\" not found, mentioned in UI interface \"ui-interface-1\"".to_string().into())
    );
}

#[test]
fn test_inconsistent_dpki() {
    let toml = r#"
[[agents]]
id = "test agent"
name = "Holo Tester 1"
public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
keystore_file = "holo_tester.key"

[[dnas]]
id = "deepkey"
file = "deepkey.dna.json"
hash = "Qm328wyq38924y"

[[cells]]
id = "deepkey"
dna = "deepkey"
agent = "test agent"
    [cells.storage]
    type = "file"
    path = "deepkey_storage"

[dpki]
cell_id = "bogus cell"
init_params = "{}"
"#;
    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");
    assert_eq!(
        config.check_consistency(&mut test_dna_loader()),
        Err(
            "cell configuration \"bogus cell\" not found, mentioned in dpki"
                .to_string()
                .into()
        )
    );
}

#[test]
fn test_check_cells_storage() -> Result<(), String> {
    let toml = r#"
    [[agents]]
    id = "test agent 1"
    keystore_file = "holo_tester.key"
    name = "Holo Tester 1"
    public_address = "HoloTester1-----------------------------------------------------------------------AAACZp4xHB"

    [[agents]]
    id = "test agent 2"
    keystore_file = "holo_tester.key"
    name = "Holo Tester 2"
    public_address = "HoloTester2-----------------------------------------------------------------------AAAGy4WW9e"

    [[cells]]
    agent = "test agent 1"
    dna = "app spec rust"
    id = "app spec cell 1"

        [cells.storage]
        path = "example-config/tmp-storage-1"
        type = "file"

    [[cells]]
    agent = "test agent 2"
    dna = "app spec rust"
    id = "app spec cell 2"

        [cells.storage]
        path = "example-config/tmp-storage-2"
        type = "file"
    "#;

    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");

    assert_eq!(config.check_cells_storage(), Ok(()));
    Ok(())
}

#[test]
fn test_check_cells_storage_err() -> Result<(), String> {
    // Here we have a forbidden duplicated 'cells.storage'
    let toml = r#"
    [[agents]]
    id = "test agent 1"
    keystore_file = "holo_tester.key"
    name = "Holo Tester 1"
    public_address = "HoloTester1-----------------------------------------------------------------------AAACZp4xHB"

    [[cells]]
    agent = "test agent 1"
    dna = "app spec rust"
    id = "app spec cell 1"

        [cells.storage]
        path = "forbidden-duplicated-storage-file-path"
        type = "file"

    [[cells]]
    agent = "test agent 2"
    dna = "app spec rust"
    id = "app spec cell 2"

        [cells.storage]
        path = "forbidden-duplicated-storage-file-path"
        type = "file"
    "#;

    let config = load_configuration::<ConductorState>(&toml)
        .expect("ConductorState should be syntactically correct");

    assert_eq!(
        config.check_cells_storage(),
        Err(String::from(
            "Forbidden duplicated file storage value encountered."
        ))
    );
    Ok(())
}
