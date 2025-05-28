#[test]
fn hdk_extern_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/hdk_extern/*.rs");
}
