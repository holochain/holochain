#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let integrity_len = hc_demo_cli::INTEGRITY_WASM.len();
    let coordinator_len = hc_demo_cli::COORDINATOR_WASM.len();
    println!("integrity_len: {integrity_len}");
    println!("coordinator_len: {coordinator_len}");

    hc_demo_cli::run_demo().await;
}
