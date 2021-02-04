use std::io::Read;

fn run_srv() -> (String, std::process::Child) {
    let mut cmd = std::process::Command::new("cargo");
    let mut cmd = cmd
        .arg("run")
        .arg("--bin")
        .arg("kitsune-p2p-proxy")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = cmd.stdout.take().unwrap();
    let mut buf = [0_u8; 4096];
    let mut out_str = String::new();
    loop {
        let size = stdout.read(&mut buf).unwrap();
        out_str.push_str(&String::from_utf8_lossy(&buf[..size]));
        if out_str.contains('\n') {
            break;
        }
    }
    out_str = out_str.split_whitespace().next().unwrap().to_string();
    (out_str, cmd)
}

fn run_cli(proxy: &str) -> (String, std::process::Child) {
    let mut cmd = std::process::Command::new("cargo");
    let mut cmd = cmd
        .arg("run")
        .arg("--bin")
        .arg("proxy-cli")
        .arg(proxy)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = cmd.stdout.take().unwrap();
    let mut out_str = String::new();
    stdout.read_to_string(&mut out_str).unwrap();
    (out_str, cmd)
}

#[tokio::test(threaded_scheduler)]
async fn srv_cli_dump_test() {
    let (proxy, mut srv) = run_srv();
    let (dump, mut cli) = run_cli(&proxy);

    cli.kill().unwrap();
    srv.kill().unwrap();

    println!("GOT DUMP:\n{}", dump);
}
