use hdk3::prelude::*;
use test_wasm_common::TestString;

#[hdk_extern]
fn foo(_: ()) -> ExternResult<TestString> {
    Ok(TestString::from(String::from("foo")))
}

#[hdk_extern]
fn bar(_: ()) -> ExternResult<TestString> {
    // It should be possible to call our extern functions just like regular functions.
    // `foo` is a blacklisted symbol name according to clippy ¯\_(ツ)_/¯
    let fo0: TestString = foo(())?;
    Ok(TestString::from(format!("{}{}", fo0.0, "bar")))
}
