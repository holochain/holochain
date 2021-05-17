use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

pub static TOKIO: Lazy<Runtime> = Lazy::new(new_runtime);

/// Instantiate a new runtime.
fn new_runtime() -> Runtime {
    // we want to use multiple threads
    tokio::runtime::Builder::new_multi_thread()
        // we use both IO and Time tokio utilities
        .enable_all()
        // give our threads a descriptive name (they'll be numbered too)
        .thread_name("holochain-tokio-thread")
        // build the runtime
        .build()
        // panic if we cannot (we cannot run without it)
        .expect("can build tokio runtime")
}

fn block_on_given<F>(f: F, runtime: &Runtime) -> F::Output
where
    F: futures::future::Future,
{
    tokio::task::block_in_place(|| runtime.block_on(async { f.await }))
}

/// Run a blocking thread on `TOKIO`.
pub fn block_on<F>(
    f: F,
    timeout: std::time::Duration,
) -> Result<F::Output, tokio::time::error::Elapsed>
where
    F: futures::future::Future,
{
    block_on_given(tokio::time::timeout(timeout, f), &TOKIO)
}

/// Run a blocking thread on `TOKIO`.
pub fn block_forever_on<F>(f: F) -> F::Output
where
    F: futures::future::Future,
{
    block_on_given(f, &TOKIO)
}

pub fn runtime_block_on<F, R>(f: F) -> F::Output
where
    F: futures::future::Future<Output = R>,
    R: Send,
{
    TOKIO.block_on(f)
}

#[cfg(test)]
mod test {

    #[tokio::test(flavor = "multi_thread")]
    async fn block_on_works() {
        crate::block_forever_on(async { println!("stdio can block") });
        assert_eq!(1, super::block_forever_on(async { 1 }));

        let r = "1";
        let test1 = super::block_forever_on(async { r.to_string() });
        assert_eq!("1", &test1);

        // - wasm style use case -
        // we are in a non-tokio context
        let test2 = std::thread::spawn(|| {
            let r = "2";
            super::block_forever_on(async { r.to_string() })
        })
        .join()
        .unwrap();
        assert_eq!("2", &test2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn block_on_allows_spawning() {
        let r = "works";
        let test =
            super::block_forever_on(tokio::task::spawn(async move { r.to_string() })).unwrap();
        assert_eq!("works", &test);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn block_on_recursive() {
        let r = "works";
        let test = super::block_forever_on(async move {
            super::block_forever_on(async move { r.to_string() })
        });
        assert_eq!("works", &test);
    }
}
