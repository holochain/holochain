use hdk::prelude::*;
use serde_yaml::Value;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((hdk::prelude::zome_info()?.name, "call_info".into()));
    functions.insert((hdk::prelude::zome_info()?.name, "remote_call_info".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

#[hdk_extern]
fn call_info(_: ()) -> ExternResult<CallInfo> {
    // Commit something here so we can show the as_at won't shift in the call
    // info returned.
    create_entry(Thing)?;
    hdk::prelude::call_info()
}

#[hdk_extern]
fn remote_call_info(agent: AgentPubKey) -> ExternResult<CallInfo> {
    match call_remote(
        agent,
        hdk::prelude::zome_info()?.name,
        "call_info".to_string().into(),
        None,
        &(),
    )? {
        ZomeCallResponse::Ok(extern_io) => Ok(extern_io.decode()?),
        not_ok => {
            tracing::warn!(?not_ok);
            Err(WasmError::Guest(format!("{:?}", not_ok)))
        },
    }
}

#[hdk_extern]
fn remote_remote_call_info(agent: AgentPubKey) -> ExternResult<CallInfo> {
    match call_remote(
        agent,
        hdk::prelude::zome_info()?.name,
        "remote_call_info".to_string().into(),
        None,
        agent_info()?.agent_initial_pubkey,
    )? {
        ZomeCallResponse::Ok(extern_io) => Ok(extern_io.decode()?),
        not_ok => {
            tracing::warn!(?not_ok);
            Err(WasmError::Guest(format!("{:?}", not_ok)))
        },
    }
}

#[hdk_extern]
fn dna_info(_: ()) -> ExternResult<DnaInfo> {
    hdk::prelude::dna_info()
}

/// `serde_yaml::Value` approach to handling properties.
/// As yaml is much more loosely typed then Rust is, everything in the yaml
/// ends up in a generic nested `Value` enum. Consider the following yaml:
///
/// foo:
///   bar: 1
///   bing: baz
///   -2: 6.0
///
/// Here we have key/value of a mapping of ints, floats, strings all in
/// positions that Rust doesn't handle particularly well. These keys and values
/// can all be present or absent. Rust would represent this as enums for every
/// key/value that can be multiple types and `Option` along with default values
/// for anything that can be absent.
///
/// For well known or relatively simple properties it may be ergonomic to
/// define a native Rust struct for the happ. For poorly defined or complex
/// properties it may be easier to use `serde_yaml::Value` directly. This does
/// several things that you will end up reinventing ad-hoc when your properties
/// become sufficiently complex:
///
/// - Defining an enum to cover all yaml types that could appear anywhere
/// - Normalised handling of floats, negative ints and ints larger than `i64::MAX`
/// - Handling floats as mapping keys
/// - Handling many optional fields without an `Option` and `Default` explosion
///
/// The main two drawbacks:
///
/// - Loss of a declarative/typed structure that can be inspected visually/IDE
/// - Inclusion of an additional dependency on `serde_yaml` in the WASM
///
/// The other option is to use `rmpv::Value` from the `rmpv` crate, but many
/// types supported by messagepack are not supported by yaml anyway. Also the
/// traversal support for moving through yaml mappings and using floats for
/// keys is relatively poor in `rmpv` compared to `serde_yaml`.
#[hdk_extern]
fn dna_info_value(k: String) -> ExternResult<serde_yaml::Value> {
    Ok(
        YamlProperties::try_from(hdk::prelude::dna_info()?.properties)?.into_inner()[k].clone()
    )
}

/// Yaml doesn't enforce the type of any value.
/// Rust can support multiple options for the type of a value as an enum.
/// Serialization will fail unless `#[serde(untagged)]` is applied to the enum
/// so that variant names are ignored.
#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
enum Foo {
    String(String),
    PosInt(u64),
}

/// Yaml files for properties can deserialize directly into a Rust struct.
/// The main benefits of taking the time to define this:
///
/// - The struct is declarative, showing how properties must be defined
/// - There are no additional dependencies if `serde_yaml::Value` is not used
#[derive(Deserialize, Serialize, Debug)]
struct PropertiesDirect {
    /// To enable a property to NOT be present it requires `#[serde(default)]`.
    /// The `Option` allows the default to simply be `None`.
    #[serde(default)]
    foo: Option<Foo>,
    /// This property can be absent but MUST be a `String` when present.
    #[serde(default)]
    bar: Option<String>,
    /// The direct struct approach does not prevent us from using `Value` in
    /// nested positions. `Value::Null` will be the default when the value is
    /// not present in the properties.
    #[serde(default)]
    baz: Value,
}

/// To support an empty properties file the entire properties struct must be
/// wrapped in an `Option` with a newtype that implements `SerializedBytes`.
#[derive(Deserialize, Serialize, Debug, SerializedBytes)]
struct MaybePropertiesDirect(Option<PropertiesDirect>);

#[hdk_extern]
fn dna_info_foo_direct(_: ()) -> ExternResult<Option<Foo>> {
    Ok(MaybePropertiesDirect::try_from(hdk::prelude::dna_info()?.properties)?.0.and_then(|properties| properties.foo))
}

#[hdk_extern]
fn dna_info_bar_direct(_: ()) -> ExternResult<Option<String>> {
    Ok(MaybePropertiesDirect::try_from(hdk::prelude::dna_info()?.properties)?.0.and_then(|properties| properties.bar))
}

#[hdk_extern]
fn dna_info_nested(_: ()) -> ExternResult<Option<i64>> {
    Ok(MaybePropertiesDirect::try_from(hdk::prelude::dna_info()?.properties)?.0.and_then(|properties| properties.baz["foo"]["bar"].as_i64()))
}

#[cfg(all(test, feature = "mock"))]
pub mod tests {
    use hdk::prelude::*;
    use ::fixt::prelude::*;

    #[test]
    fn zome_info_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let output = fixt!(ZomeInfo);
        let output_closure = output.clone();
        mock_hdk.expect_zome_info()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::zome_info(());

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}