/*
use holochain_trace::{span_context, Context, OpenSpanExt};
use tokio::sync::mpsc;
use tracing::*;

#[tokio::test(threaded_scheduler)]
async fn same_thread_test() {
    holochain_trace::test_run_open().ok();
    let span = debug_span!("span a");
    let context = span.get_context();
    let _g = span.enter();

    span_context!(span, Level::DEBUG);
    debug!(msg = "in span a");

    let span = debug_span!("span b");
    let _g = span.enter();
    debug!("in span b");
    span_context!(span, Level::DEBUG);

    let span = debug_span!("span c");
    span.set_context(context);
    span_context!(span, Level::DEBUG);
    let _g = span.enter();
    debug!("in span c");
}

#[tokio::test(threaded_scheduler)]
async fn cross_thread_test() {
    holochain_trace::test_run_open().ok();
    let (mut tx1, rx1) = mpsc::channel(100);
    let (tx2, mut rx2) = mpsc::channel(100);
    tokio::task::spawn(across_thread(rx1, tx2));
    {
        let span = debug_span!("from original thread");
        let context = span.get_context();
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        tx1.send(context).await.unwrap();
    }
    {
        let context = rx2.recv().await.unwrap();

        let span = debug_span!("original thread");
        span.set_context(context);
        span_context!(span, Level::DEBUG);
        let _g = span.enter();
        let span = debug_span!("inner");
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
    }
    {
        let context = rx2.recv().await.unwrap();

        let span = debug_span!("original thread");
        span.set_context(context);
        span_context!(span, Level::DEBUG);
        let _g = span.enter();
    }
}

async fn across_thread(mut rx: mpsc::Receiver<Context>, mut tx: mpsc::Sender<Context>) {
    {
        let context = rx.recv().await.unwrap();
        let span = debug_span!("across thread");
        span.set_context(context);
        span_context!(span, Level::DEBUG);
        let _g = span.enter();
        let span = debug_span!("inner");
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        tx.send(span.get_context()).await.unwrap();
    }
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    {
        let span = debug_span!("from another thread");
        let context = span.get_context();
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        tx.send(context).await.unwrap();
    }
}
*/
