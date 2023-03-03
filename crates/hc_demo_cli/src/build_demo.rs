#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_demo");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_demo");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_demo";

/// hc_demo_cli integrity wasm bytes
pub const INTEGRITY_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/integrity/wasm32-unknown-unknown/release/hc_demo_cli.wasm"));

/// hc_demo_cli coordinator wasm bytes
pub const COORDINATOR_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coordinator/wasm32-unknown-unknown/release/hc_demo_cli.wasm"));

/// Execute the demo
pub async fn run_demo() {
    let mut bs_addr = None;

    for addr in tokio::net::lookup_host("bootstrap.holo.host:443").await.unwrap() {
        if addr.ip().is_ipv4() {
            bs_addr = Some(addr);
            break;
        }
    }

    let mut sig_addr = None;

    for addr in tokio::net::lookup_host("holotest.net:443").await.unwrap() {
        if addr.ip().is_ipv4() {
            sig_addr = Some(addr);
            break;
        }
    }

    println!("{bs_addr:?} {sig_addr:?}");

    struct PubRendezvous {
        bs_addr: std::net::SocketAddr,
        sig_addr: std::net::SocketAddr,
    }

    impl holochain::sweettest::SweetRendezvous for PubRendezvous {
        fn bootstrap_addr(&self) -> std::net::SocketAddr {
            self.bs_addr
        }

        fn turn_addr(&self) -> &str {
            ""
        }

        fn sig_addr(&self) -> std::net::SocketAddr {
            self.sig_addr
        }
    }

    let _r = PubRendezvous {
        bs_addr: bs_addr.unwrap(),
        sig_addr: sig_addr.unwrap(),
    };

    let _c = holochain::sweettest::SweetConductorConfig::standard();
}
