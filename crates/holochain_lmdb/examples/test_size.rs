use std::path::Path;
use std::sync::Arc;

use hl::prelude::*;
use holochain_lmdb as hl;

#[tokio::main(threaded_scheduler)]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1) {
        Some(path) => {
            let env =
                EnvironmentWrite::new(Path::new(path), EnvironmentKind::Wasm, test_keystore())
                    .unwrap();
            let info = env.guard().rkv().info().unwrap().map_size();
            let size = info / 1_000_000;
            println!("Map size: {}MB", size);
        }
        None => {
            let env = test_wasm_env();
            let tmp = env.tmpdir();
            let info = env.env().guard().rkv().info().unwrap().map_size();
            let size = info / 1_000_000;
            drop(env);
            let tmp = Arc::try_unwrap(tmp).unwrap();
            let p = tmp.into_path();
            println!("Map size: {}MB Path: {}", size, p.display());
        }
    }
}
