use hdk::prelude::*;

#[hdk_extern]
fn foo(_: ()) -> ExternResult<String> {
    Ok(String::from("foo"))
}

#[hdk_extern]
fn bar(_: ()) -> ExternResult<String> {
    // It should be possible to call our extern functions just like regular functions.
    #[allow(clippy::blacklisted_name)]
    let foo: String = foo(())?;
    Ok(format!("{}{}", foo, "bar"))
}

#[hdk_extern(infallible)]
fn infallible(_: ()) -> String {
    String::from("infallible")
}