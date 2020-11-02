use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

pub static TOKIO: Lazy<Runtime> = Lazy::new(new_runtime);

/// Instantiate a new runtime.
pub fn new_runtime() -> Runtime {
    // we want to use multiple threads
    tokio::runtime::Builder::new_multi_thread()
        // we use both IO and Time tokio utilities
        .enable_all()
        // we want to use thread count matching cpu count
        // (sometimes tokio by default only uses half cpu core threads)
        .worker_threads(num_cpus::get())
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
    let _g = runtime.enter();
    tokio::task::block_in_place(|| runtime.block_on(async { f.await }))
}

/// Run a blocking thread on `TOKIO`.
pub fn block_on<F>(f: F) -> F::Output
where
    F: futures::future::Future,
{
    block_on_given(f, &TOKIO)
}

/// Run a blocking thread on a new runtime.
pub fn block_on_new<F>(f: F) -> F::Output
where
    F: futures::future::Future,
{
    block_on_given(f, &new_runtime())
}

#[cfg(test)]
mod test {

    #[tokio::test(flavor = "multi_thread")]
    async fn block_on_works() {
        crate::block_on(async { println!("stdio can block") });
        assert_eq!(1, super::block_on(async { 1 }));

        let r = "1";
        let test1 = super::block_on(async { r.to_string() });
        assert_eq!("1", &test1);

        // - wasm style use case -
        // we are in a non-tokio context
        let test2 = std::thread::spawn(|| {
            let r = "2";
            super::block_on(async { r.to_string() })
        })
        .join()
        .unwrap();
        assert_eq!("2", &test2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn block_on_allows_spawning() {
        let r = "works";
        let test = super::block_on(tokio::task::spawn(async move { r.to_string() })).unwrap();
        assert_eq!("works", &test);
    }
}
