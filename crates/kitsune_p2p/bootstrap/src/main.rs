#[tokio::main(flavor = "multi_thread")]
async fn main() {
    match kitsune_p2p_bootstrap::run(([0, 0, 0, 0], 0)).await {
        Ok((driver, addr)) => {
            println!("http://{}", addr);
            driver.await;
        }
        Err(err) => eprintln!("{}", err),
    }
}
