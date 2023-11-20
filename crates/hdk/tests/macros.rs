#[test]
fn macros() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/macros/*.rs");
}
