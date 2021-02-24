//! More ergonomic frontend structs for actually using implemented tx2 backends.

use crate::tx2::tx_backend::*;
use crate::tx2::util::TxUrl;
use crate::*;

/// A factory that allows binding endpoints
pub struct TxEndpointFramedFactory<B>(std::marker::PhantomData<B>)
where
    B: BackendAdapt;

impl<B> Default for TxEndpointFramedFactory<B>
where
    B: BackendAdapt,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<B> TxEndpointFramedFactory<B>
where
    B: BackendAdapt,
{
    /// Construct a new factory with given concrete backend type.
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }

    /// Bind a new endpoint with the captured concrete backend type.
    pub async fn bind<U>(&self, url: U, timeout: KitsuneTimeout) -> KitsuneResult<TxEndpointFramed>
    where
        U: Into<TxUrl>,
    {
        let ep_pair = B::bind(url.into(), timeout).await?;
        TxEndpointFramed::new(ep_pair)
    }
}

/// Struct representing a bound local endpoint for connections.
/// This layer adds connection / channel management and framing.
/// See TxEndpointCodec for the high-level interface.
pub struct TxEndpointFramed {}

impl TxEndpointFramed {
    /// Construct a new instance from given endpoint pair.
    pub fn new(ep_pair: Endpoint) -> KitsuneResult<Self> {
        let (_ep, con_recv) = ep_pair;
        let con_recv = futures::stream::unfold(con_recv, |mut con_recv| async move {
            let item = match con_recv.next().await {
                Ok(item) => item,
                Err(_) => return None,
            };
            Some((item, con_recv))
        });
        // TODO - configurable?
        let _con_recv = futures::stream::StreamExt::buffer_unordered(con_recv, 32);
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx2::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx_endpoint_framed() {
        let factory = <TxEndpointFramedFactory<MemBackendAdapt>>::new();
        let _ = factory.bind("none:", KitsuneTimeout::from_millis(1000 * 30));
    }
}
