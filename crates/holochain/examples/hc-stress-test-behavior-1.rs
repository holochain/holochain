use holochain::test_utils::hc_stress_test::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let test = LocalBehavior1::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        println!("{:#?}", &*test.lock().unwrap());
    }
}
