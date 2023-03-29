#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_demo");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_demo");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_demo";

use hdk::prelude::*;
super::wasm_common!();

/// hc_demo_cli integrity wasm bytes
pub const INTEGRITY_WASM_GZ: &[u8] = include_bytes!("integrity.wasm.gz");

/// hc_demo_cli coordinator wasm bytes
pub const COORDINATOR_WASM_GZ: &[u8] = include_bytes!("coordinator.wasm.gz");

use holochain_types::prelude::*;
use std::sync::Arc;

/// `hc demo-cli` - Self-contained demo for holochain functionality.
///
/// First, you need to save a dna file to use with the demo:
///
/// `hc demo-cli gen-dna-file --output my.dna`
///
/// Then, distribute that dna file to other systems, and run:
///
/// `hc demo-cli run --dna my.dna`
///
/// The demo will create two directories: `hc-demo-cli-inbox` and
/// `hc-demo-cli-outbox`. Put files into the inbox, and they will
/// be published to the network. All files discovered on the network
/// will be written to the outbox.
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
pub struct RunOpts {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: RunCmd,
}

impl RunOpts {
    /// Parse command-line arguments into a RunOpts instance.
    pub fn parse() -> Self {
        clap::Parser::parse()
    }
}

/// hc_demo_cli run command.
#[derive(Debug, clap::Subcommand, serde::Serialize, serde::Deserialize)]
pub enum RunCmd {
    /// Run the hc demo-cli.
    Run {
        /// The dna file path. Default "-" for stdin.
        #[arg(long, default_value = "-")]
        dna: std::path::PathBuf,

        /// The inbox path.
        #[arg(long, default_value = "hc-demo-cli-inbox")]
        inbox: std::path::PathBuf,

        /// the outbox path.
        #[arg(long, default_value = "hc-demo-cli-outbox")]
        outbox: std::path::PathBuf,
    },

    /// Generate a dna file that can be used with hc demo-cli.
    GenDnaFile {
        /// Filename path to write the dna file. Default "-" for stdout.
        #[arg(long, default_value = "-")]
        output: std::path::PathBuf,
    },
}

/// Execute the demo
pub async fn run_demo(opts: RunOpts) {
    tracing::info!(?opts);
    match opts.command {
        RunCmd::Run { dna, inbox, outbox } => {
            run(dna, inbox, outbox, None, None).await;
        }
        RunCmd::GenDnaFile { output } => {
            gen_dna_file(output).await;
        }
    }
}

#[cfg(test)]
pub async fn run_test_demo(
    opts: RunOpts,
    ready: tokio::sync::oneshot::Sender<()>,
    rendezvous: holochain::sweettest::DynSweetRendezvous,
) {
    tracing::info!(?opts);
    match opts.command {
        RunCmd::Run { dna, inbox, outbox } => {
            run(dna, inbox, outbox, Some(ready), Some(rendezvous)).await;
        }
        RunCmd::GenDnaFile { output } => {
            gen_dna_file(output).await;
        }
    }
}

async fn gen_dna_file(output: std::path::PathBuf) {
    let mut i_wasm = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::GzDecoder::new(std::io::Cursor::new(INTEGRITY_WASM_GZ)),
        &mut i_wasm,
    )
    .unwrap();

    let i_wasm = DnaWasmHashed::from_content(DnaWasm {
        code: Arc::new(i_wasm.into_boxed_slice()),
    })
    .await;
    let i_zome = IntegrityZomeDef::from(ZomeDef::Wasm(WasmZome::new(i_wasm.hash.clone())));

    let mut c_wasm = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::GzDecoder::new(std::io::Cursor::new(COORDINATOR_WASM_GZ)),
        &mut c_wasm,
    )
    .unwrap();

    let c_wasm = DnaWasmHashed::from_content(DnaWasm {
        code: Arc::new(c_wasm.into_boxed_slice()),
    })
    .await;
    let c_zome = CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome::new(c_wasm.hash.clone())));

    let network_seed = rand_utf8::rand_utf8(&mut rand::thread_rng(), 32);

    let dna_def = DnaDefBuilder::default()
        .name("hc_demo_cli".to_string())
        .modifiers(
            DnaModifiersBuilder::default()
                .network_seed(network_seed.into())
                .origin_time(Timestamp::now())
                .build()
                .unwrap(),
        )
        .integrity_zomes(vec![("integrity".into(), i_zome)])
        .coordinator_zomes(vec![("coordinator".into(), c_zome)])
        .build()
        .unwrap();

    let dna_file = DnaFile::new(dna_def, vec![i_wasm.into_content(), c_wasm.into_content()]).await;

    let dna_file: SerializedBytes = dna_file.try_into().unwrap();
    let dna_file: UnsafeBytes = dna_file.try_into().unwrap();
    let dna_file: Vec<u8> = dna_file.into();

    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut gz, &dna_file).unwrap();
    let dna_file = gz.finish().unwrap();

    if output == std::path::PathBuf::from("-") {
        tokio::io::AsyncWriteExt::write_all(&mut tokio::io::stdout(), &dna_file)
            .await
            .unwrap();
    } else {
        tokio::fs::write(output, &dna_file).await.unwrap();
    }
}

async fn run(
    dna: std::path::PathBuf,
    inbox: std::path::PathBuf,
    outbox: std::path::PathBuf,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    rendezvous: Option<holochain::sweettest::DynSweetRendezvous>,
) {
    let _ = tokio::fs::create_dir_all(&inbox).await;
    let _ = tokio::fs::create_dir_all(&outbox).await;

    let dna_gz = if dna.to_string_lossy() == "-" {
        let mut dna_gz = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut tokio::io::stdin(), &mut dna_gz)
            .await
            .unwrap();
        dna_gz
    } else {
        tokio::fs::read(dna).await.unwrap()
    };

    let mut dna = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::GzDecoder::new(std::io::Cursor::new(dna_gz)),
        &mut dna,
    )
    .unwrap();

    let dna: UnsafeBytes = dna.into();
    let dna: SerializedBytes = dna.try_into().unwrap();
    let dna: DnaFile = dna.try_into().unwrap();

    let rendezvous = match rendezvous {
        Some(rendezvous) => rendezvous,
        None => {
            struct PubRendezvous;

            impl holochain::sweettest::SweetRendezvous for PubRendezvous {
                fn bootstrap_addr(&self) -> &str {
                    "https://bootstrap.holo.host"
                }

                fn sig_addr(&self) -> &str {
                    "wss://holotest.net"
                }
            }

            let rendezvous: holochain::sweettest::DynSweetRendezvous = Arc::new(PubRendezvous);
            rendezvous
        }
    };

    let config = holochain::sweettest::SweetConductorConfig::standard();

    let keystore = holochain_keystore::spawn_mem_keystore().await.unwrap();

    let mut conductor = holochain::sweettest::SweetConductor::from_config_rendezvous_keystore(
        config, rendezvous, keystore,
    )
    .await;

    let dna_with_role = holochain::sweettest::DnaWithRole::from(("hc_demo_cli".into(), dna));

    let app = conductor
        .setup_app("hc_demo_cli", vec![&dna_with_role])
        .await
        .unwrap();

    let cell = app.cells().get(0).unwrap().clone();
    tracing::info!(?cell);

    // PRINT to stdout instead of trace
    println!("#DNA_HASH#{}#", cell.dna_hash());
    println!("#AGENT_KEY#{}#", cell.agent_pubkey());

    let i_zome = cell.zome("integrity");
    tracing::info!(?i_zome);
    let c_zome = cell.zome("coordinator");
    tracing::info!(?c_zome);

    let handle = conductor.sweet_handle();

    if let Some(ready) = ready {
        let _ = ready.send(());
    }

    loop {
        let mut dir = tokio::fs::read_dir(&inbox).await.unwrap();

        while let Ok(Some(i)) = dir.next_entry().await {
            if i.file_type().await.unwrap().is_file() {
                let name = i.path();
                let name = name.file_name().unwrap().to_string_lossy().to_string();
                let data = tokio::fs::read(i.path()).await.unwrap();

                if data.len() > 2 * 1024 * 1024 {
                    panic!("{:?}: file too large: max 2MiB for hc demo-cli", i.path());
                }

                tracing::info!(?name, byte_count = %data.len());

                let create_file: Record = handle
                    .call(
                        &c_zome,
                        "create_file",
                        File {
                            desc: name.clone(),
                            data: UnsafeBytes::from(data).try_into().unwrap(),
                        },
                    )
                    .await;
                tracing::info!(?create_file);

                tokio::fs::remove_file(i.path()).await.unwrap();

                println!("#PUBLISHED#{name}#");
            }
        }

        let all_files: Vec<ActionHash> = handle.call(&c_zome, "get_all_files", ()).await;

        for file in all_files {
            let mut path = outbox.clone();
            path.push(file.to_string());

            if tokio::fs::metadata(&path).await.is_ok() {
                continue;
            }

            let data: File = match handle.call(&c_zome, "get_file", file.clone()).await {
                Some(data) => data,
                None => {
                    println!("#WARN#failed to fetch hash {file}, waiting 5 seconds#");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let _ = tokio::fs::create_dir_all(&path).await;

            path.push(&data.desc);

            let bytes: UnsafeBytes = data.data.try_into().unwrap();
            let bytes: Vec<u8> = bytes.into();
            tokio::fs::write(&path, &bytes).await.unwrap();

            println!("#FETCHED#{path:?}#");
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // conductor.shutdown().await;
}
