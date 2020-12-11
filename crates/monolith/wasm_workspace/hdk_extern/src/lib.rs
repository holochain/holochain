use hdk3::prelude::*;
use holochain_test_wasm_common::TestString;

#[hdk_extern]
fn foo(_: ()) -> ExternResult<TestString> {
    Ok(TestString::from(String::from("foo")))
}

#[hdk_extern]
fn bar(_: ()) -> ExternResult<TestString> {
    // It should be possible to call our extern functions just like regular functions.
    #[allow(clippy::blacklisted_name)]
    let foo: TestString = foo(())?;
    Ok(TestString::from(format!("{}{}", foo.0, "bar")))
}
