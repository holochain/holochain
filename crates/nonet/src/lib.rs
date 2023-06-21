#[tokio::test]
pub async fn test_nonet() -> anyhow::Result<()> {
    match reqwest::get("https://www.rust-lang.org").await.error_for_status() {
        Ok(_response) => {
            #[cfg(feature = "nonet")]
            panic!("Connected to the internet with nonet feature enabled.");
        }
        Err(_err) => {
            #[cfg(not(feature = "nonet"))]
            panic!("Failed to connect to the internet without nonet feature enabled (Yes, that's a triple negative).");
        }
    }
    Ok(())
}