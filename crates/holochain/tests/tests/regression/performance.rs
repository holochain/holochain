use hdk::prelude::Record;
use holo_hash::ActionHash;
use holochain::{
    sweettest::{SweetConductor, SweetConductorConfig, SweetDnaFile},
    test_utils::inline_zomes::simple_create_read_zome,
};
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn p() {
    let config = SweetConductorConfig::standard();
    // config.data_root_path
    let mut conductor = SweetConductor::from_config(config).await;
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("cr", simple_create_read_zome())).await;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();

    let start = Instant::now();
    let mut count = 0;
    let max_count = 5000;
    let mut avg_count_duration = Duration::from_secs(0);
    let checkpoint = 100;

    while count < max_count {
        let call_start = Instant::now();
        let action_hash: ActionHash = conductor
            .call(&app.cells()[0].zome("cr"), "create", ())
            .await;
        let _record: Record = conductor
            .call(&app.cells()[0].zome("cr"), "read", action_hash)
            .await;
        avg_count_duration += call_start.elapsed();
        count += 1;
        if count % checkpoint == 0 {
            println!(
                "The last {checkpoint} calls took on average {} ms",
                avg_count_duration.as_millis() / checkpoint
            );
            avg_count_duration = Duration::from_secs(0);
        }

        if count % 10_000 == 0 {
            println!("Made {count} calls.");
        }
    }

    println!("Made {count} calls in total over {:?}.", start.elapsed());
}
