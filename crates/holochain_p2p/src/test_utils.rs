use tokio::time::error::Elapsed;

/// Try a function, with pauses between retries, until it returns `true` or the timeout duration elapses.
/// The default timeout is 5 s.
/// The default pause is 500 ms.
pub async fn retry_fn_until_timeout<F, Fut>(
    try_fn: F,
    timeout_ms: Option<u64>,
    sleep_ms: Option<u64>,
) -> Result<(), Elapsed>
where
    F: Fn() -> Fut,
    Fut: core::future::Future<Output = bool>,
{
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.unwrap_or(5000)),
        async {
            loop {
                if try_fn().await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms.unwrap_or(500))).await;
            }
        },
    )
    .await
}
