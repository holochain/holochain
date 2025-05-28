use hdk::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct TestString(pub String);

#[hdk_extern]
fn foo(_: ()) -> ExternResult<TestString> {
    Ok(TestString("foo".to_string()))
}

#[hdk_extern]
fn bar(_: ()) -> ExternResult<TestString> {
    Ok(TestString("bar".to_string()))
}

#[hdk_extern]
fn emitter(_: ()) -> ExternResult<TestString> {
    match emit_signal(&TestString("i am a signal".to_string())) {
        Ok(()) => Ok(TestString("bar".to_string())),
        Err(e) => Err(e),
    }
}
