const NOTICE: &str = r#"--- hc_demo_cli wasm ---
If this test fails, you may need to recompile the wasm:
cd crates/hc_demo_cli
RUSTFLAGS="--cfg build_wasm" cargo build
--- hc_demo_cli wasm ---"#;

#[tokio::test(flavor = "multi_thread")]
async fn demo() {
    eprintln!("{NOTICE}");

    panic!("nope");
}
