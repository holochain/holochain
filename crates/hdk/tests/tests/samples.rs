#[test]
#[cfg_attr(feature = "fixturators", ignore)] // Turns on `full-dna-def` in holochain_zome_types which turns on `kitsune_p2p_timestamp/now`.
fn samples() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/samples/*.rs");
}
