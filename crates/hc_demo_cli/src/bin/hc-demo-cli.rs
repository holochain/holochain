fn init_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    init_tracing();

    hc_demo_cli::run_demo(hc_demo_cli::RunOpts::parse()).await;
}
