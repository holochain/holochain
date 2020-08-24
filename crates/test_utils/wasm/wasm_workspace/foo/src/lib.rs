use hdk3::prelude::*;
use test_wasm_common::TestString;

#[hdk_extern]
fn foo(_: ()) -> ExternResult<TestString> {
    Ok(TestString::from(String::from("foo")))
}
