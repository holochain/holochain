#[test]
fn samples() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/samples/*.rs");
}
