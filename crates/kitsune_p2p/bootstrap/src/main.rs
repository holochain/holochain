use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx, rx) = oneshot::channel();
    kitsune_bootstrap::run(([127, 0, 0, 1], 0), tx).await;
    let addr = rx.await;
    if let Ok(addr) = addr {
        println!("Connected to {:?}", addr);
    }
}
