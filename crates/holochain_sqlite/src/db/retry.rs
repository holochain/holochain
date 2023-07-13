use std::future::Future;

#[derive(Debug)]
pub struct OptimisticRetryError<E: std::error::Error>(Vec<E>);

impl<E: std::error::Error> std::fmt::Display for OptimisticRetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "OptimisticRetryError had too many failures:\n{:#?}",
            self.0
        )
    }
}

impl<E: std::error::Error> std::error::Error for OptimisticRetryError<E> {}

pub async fn optimistic_retry_async<Func, Fut, T, E>(
    ctx: &str,
    mut f: Func,
) -> Result<T, OptimisticRetryError<E>>
where
    Func: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    use tokio::time::Duration;
    const NUM_CONSECUTIVE_FAILURES: usize = 10;
    const RETRY_INTERVAL: Duration = Duration::from_millis(500);
    let mut errors = Vec::new();
    loop {
        match f().await {
            Ok(x) => return Ok(x),
            Err(err) => {
                tracing::error!(
                    "Error during optimistic_retry. Failures: {}/{}. Context: {}. Error: {:?}",
                    errors.len() + 1,
                    NUM_CONSECUTIVE_FAILURES,
                    ctx,
                    err
                );
                errors.push(err);
                if errors.len() >= NUM_CONSECUTIVE_FAILURES {
                    return Err(OptimisticRetryError(errors));
                }
            }
        }
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}
