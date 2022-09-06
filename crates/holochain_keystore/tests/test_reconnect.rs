use assert_cmd::cargo::CommandCargoExt;
use holochain_keystore::lair_keystore::*;
use holochain_keystore::MetaLairClient;
use kitsune_p2p_types::dependencies::url2;
use std::io::BufRead;
use std::sync::Arc;

struct Proc(std::process::Child);

impl Drop for Proc {
    fn drop(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

struct Cli(MetaLairClient);

impl Drop for Cli {
    fn drop(&mut self) {
        let fut = self.0.shutdown();
        tokio::task::spawn(fut);
    }
}

impl std::ops::Deref for Cli {
    type Target = MetaLairClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn run_test_keystore(dir: &std::path::Path) -> (Proc, url2::Url2) {
    let mut cmd = std::process::Command::cargo_bin("test-keystore-srv").unwrap();
    cmd.arg(dir).stdout(std::process::Stdio::piped());

    println!("{:?}", cmd);

    let mut cmd = cmd.spawn().unwrap();

    let mut yaml = String::new();
    let mut lines = std::io::BufReader::new(cmd.stdout.take().unwrap()).lines();
    while let Some(line) = lines.next() {
        let line = line.unwrap();
        if line == "OK" {
            break;
        }
        yaml.push_str(&line);
        yaml.push('\n');
    }

    tokio::task::spawn(async move { for _line in lines {} });

    #[derive(Debug, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Conf {
        connection_url: url2::Url2,
    }

    let conf: Conf = serde_yaml::from_str(&yaml).unwrap();

    (Proc(cmd), conf.connection_url)
}

async fn connect_cli(connection_url: url2::Url2) -> Cli {
    let passphrase = sodoken::BufRead::from(&b"passphrase"[..]);
    let cli = spawn_lair_keystore(connection_url, passphrase)
        .await
        .unwrap();

    Cli(cli)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reconnect() {
    let tmpdir = tempdir::TempDir::new("lair keystore test").unwrap();
    let tag: Arc<str> = "test-tag".into();

    let start = std::time::Instant::now();

    let (proc, url) = run_test_keystore(tmpdir.path());
    let cli = connect_cli(url).await;
    cli.get_or_create_tls_cert_by_tag(tag.clone())
        .await
        .unwrap();

    println!("launch to first test call in {:?}", start.elapsed());

    drop(proc);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert!(cli
        .get_or_create_tls_cert_by_tag(tag.clone())
        .await
        .is_err());

    let (proc, _url) = run_test_keystore(tmpdir.path());

    let mut all_good = false;

    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if cli.get_or_create_tls_cert_by_tag(tag.clone()).await.is_ok() {
            all_good = true;
            break;
        }
    }

    drop(cli);
    drop(proc);

    if !all_good {
        panic!("Reconnect was never successful");
    }
}
