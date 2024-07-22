#[test]
#[ignore = "this is a long-running test that slows down ci - execute this manually"]
fn macros() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/macros/*.rs");
}
