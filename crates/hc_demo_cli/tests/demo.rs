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

    tokio::fs::create_dir_all("one-in").await.unwrap();
    tokio::fs::create_dir_all("two-out").await.unwrap();
    tokio::fs::write(format!("one-in/{FILE}"), CONTENT)
        .await
        .unwrap();

    let t1 = tokio::task::spawn(run("one"));
    let t2 = tokio::task::spawn(run("two"));
    let t3 = tokio::task::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        panic!("Failed to tx file in 60 seconds");
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut nodes = tokio::fs::read_dir("two-out").await.unwrap();
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
    let opts = hc_demo_cli::RunOpts {
        command: hc_demo_cli::RunCmd::GenDnaFile {
            output: std::path::PathBuf::from(DNA),
        },
    };

    hc_demo_cli::run_demo(opts).await;
}

async fn run(name: &str) {
    let opts = hc_demo_cli::RunOpts {
        command: hc_demo_cli::RunCmd::Run {
            dna: std::path::PathBuf::from(DNA),
            inbox: std::path::PathBuf::from(format!("{name}-in")),
            outbox: std::path::PathBuf::from(format!("{name}-out")),
        },
    };

    hc_demo_cli::run_demo(opts).await;
}
