//! Helpers and utilities

use must_future::MustBoxFuture;

/// Helper function for the common case of returning this nested Unit type.
pub fn unit_ok_fut<E1, E2>() -> Result<MustBoxFuture<'static, Result<(), E2>>, E1> {
    use futures::FutureExt;
    Ok(async move { Ok(()) }.boxed().into())
}

/// Helper function for the common case of returning this boxed future type.
pub fn ok_fut<E1, R: Send + 'static>(result: R) -> Result<MustBoxFuture<'static, R>, E1> {
    use futures::FutureExt;
    Ok(async move { result }.boxed().into())
}

/// Helper function for the common case of returning this boxed future type.
pub fn box_fut<'a, R: Send + 'a>(result: R) -> MustBoxFuture<'a, R> {
    use futures::FutureExt;
    async move { result }.boxed().into()
}

