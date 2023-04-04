use super::*;

const NOTICE: &str = r#"--- hc_demo_cli wasm ---
If this test fails, you may need to recompile the wasm:
cd crates/hc_demo_cli
RUSTFLAGS="--cfg build_wasm" cargo build
--- hc_demo_cli wasm ---"#;

const DNA: &str = "dna.gz";
const FILE: &str = "test.txt";
const CONTENT: &[u8] = b"this is a test\n";

#[tokio::test(flavor = "multi_thread")]
async fn demo() {
    init_tracing();

    eprintln!("{NOTICE}");

    let tmp = tempfile::tempdir().unwrap();
    println!("{tmp:?}");
    std::env::set_current_dir(&tmp).unwrap();

    gen_dna().await;

    tokio::fs::create_dir_all("one-out").await.unwrap();
    tokio::fs::create_dir_all("two-in").await.unwrap();
    tokio::fs::write(format!("one-out/{FILE}"), CONTENT)
        .await
        .unwrap();

    let (r1s, r1r) = tokio::sync::oneshot::channel();
    let (r2s, r2r) = tokio::sync::oneshot::channel();
    let rendezvous = holochain::sweettest::SweetLocalRendezvous::new().await;

    let t1 = tokio::task::spawn(run("one", r1s, rendezvous.clone()));
    let t2 = tokio::task::spawn(run("two", r2s, rendezvous.clone()));

    let _ = r1r.await;
    let _ = r2r.await;

    let t3 = tokio::task::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        panic!("Failed to tx file in 60 seconds");
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut nodes = tokio::fs::read_dir("two-in").await.unwrap();
        while let Some(node) = nodes.next_entry().await.unwrap() {
            if !node.file_type().await.unwrap().is_dir() {
                continue;
            }

            let mut file = node.path();
            file.push(FILE);

            let content = match tokio::fs::read(file).await {
                Err(_) => continue,
                Ok(content) => content,
            };

            assert_eq!(content, CONTENT);

            t1.abort();
            t2.abort();
            t3.abort();

            // allow some time to close file handles
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let _ = tmp.close();
            return;
        }
    }
}

fn init_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

async fn gen_dna() {
    let opts = RunOpts {
        command: RunCmd::GenDnaFile {
            output: std::path::PathBuf::from(DNA),
        },
    };

    run_demo(opts).await;
}

async fn run(
    name: &str,
    ready: tokio::sync::oneshot::Sender<()>,
    rendezvous: holochain::sweettest::DynSweetRendezvous,
) {
    let opts = RunOpts {
        command: RunCmd::Run {
            dna: std::path::PathBuf::from(DNA),
            outbox: std::path::PathBuf::from(format!("{name}-out")),
            inbox: std::path::PathBuf::from(format!("{name}-in")),
        },
    };

    run_test_demo(opts, ready, rendezvous).await;
}
