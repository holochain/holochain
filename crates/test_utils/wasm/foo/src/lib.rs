use hdk3::prelude::*;
use test_wasm_common::TestString;

#[hdk(extern)]
fn foo(_: ()) -> ExternResult<TestString> {
    Ok(TestString::from(String::from("foo")))
}
