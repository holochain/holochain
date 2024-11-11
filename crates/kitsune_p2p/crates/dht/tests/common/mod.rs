pub mod continuous;
pub mod quantized;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .try_init()
        .ok();
}
