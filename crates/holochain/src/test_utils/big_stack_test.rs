/// Wrap a test body in this macro to give it a bigger stack. Expects the body to
/// be async.
#[macro_export]
macro_rules! big_stack_test {
    ($what_do:expr, $size:literal) => {
        tokio::runtime::Builder::new_multi_thread()
            // Need a bigger stack for this test for some reason.
            .thread_stack_size($size)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                // This spawns a task with a big stack. The outer block on does
                // NOT have the larger stack size.
                tokio::task::spawn(tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    $what_do,
                ))
                .await
                .unwrap()
            })
            .unwrap()
    };

    ($what_do:expr) => {
        $crate::big_stack_test!($what_do, 11_000_000)
    };
}
