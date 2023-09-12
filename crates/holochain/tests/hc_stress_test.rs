#[tokio::test(flavor = "multi_thread")]
async fn hc_stress_test_two_nodes() {
    // this is a stub right now... it always errors
    assert!(holochain::test_utils::hc_stress_test::hc_stress_test_two_nodes().await.is_err());
}
