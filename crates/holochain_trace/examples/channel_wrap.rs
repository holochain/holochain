fn main() {}
/*
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "channels")]
    wrap_it::run().await?;
    Ok(())
}

#[cfg(feature = "channels")]
mod wrap_it {
    use super::*;

    use holochain_trace::{channel::mpsc, span_context};
    use tracing::*;

    #[derive(Debug)]
    struct Foo;

    struct MyChannel {
        rx: mpsc::Receiver<Foo>,
        tx: mpsc::Sender<Foo>,
    }

    impl MyChannel {
        fn new(tx: mpsc::Sender<Foo>, rx: mpsc::Receiver<Foo>) -> Self {
            Self { rx, tx }
        }
    }

    pub async fn run() -> Result<(), Box<dyn Error>> {
        holochain_trace::test_run_open().ok();
        let (tx1, rx2) = mpsc::channel(10);
        let (tx2, rx1) = mpsc::channel(10);
        let c1 = MyChannel::new(tx1.clone(), rx1);
        let c2 = MyChannel::new(tx2, rx2);
        let (tx4, rx4) = mpsc::channel(10);
        let (_, dead) = mpsc::channel(1);
        let c3 = MyChannel::new(tx1, rx4);
        let c4 = MyChannel::new(tx4, dead);
        let mut jh = Vec::new();
        jh.push(tokio::task::spawn(async { a(c1).await.unwrap() }));
        jh.push(tokio::task::spawn(async { b(c2, c4).await.unwrap() }));
        jh.push(tokio::task::spawn(async { c(c3).await.unwrap() }));

        for h in jh {
            h.await?;
        }
        Ok(())
    }

    #[instrument(skip(channel))]
    async fn a(mut channel: MyChannel) -> Result<(), Box<dyn Error>> {
        for _ in 0..10 {
            span_context!(Span::current());
            channel.tx.send(Foo).await?;
            if let Some(_) = channel.rx.recv().await {}
        }
        tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    #[instrument(skip(channel, to_c))]
    async fn b(mut channel: MyChannel, mut to_c: MyChannel) -> Result<(), Box<dyn Error>> {
        for _ in 0..10 {
            span_context!(Span::current());
            if let Some(_) = channel.rx.recv().await {}
            channel.tx.send(Foo).await?;
            to_c.tx.send(Foo).await?;
        }
        tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    #[instrument(skip(from_b_to_a))]
    async fn c(mut from_b_to_a: MyChannel) -> Result<(), Box<dyn Error>> {
        for _ in 0..10 {
            span_context!(Span::current());
            if let Some(_) = from_b_to_a.rx.recv().await {}
            from_b_to_a.tx.send(Foo).await?;
        }
        tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        Ok(())
    }
}
*/
