//! automated behavioral testing of hc-stress-test zomes

use crate::sweettest::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::*;

fn uid() -> i64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// A conductor running the hc_stress_test app.
pub struct HcStressTest {
    conductor: SweetConductor,
    cell: SweetCell,
}

impl HcStressTest {
    /// Helper to provide the SweetDnaFile from compiled test wasms.
    pub async fn test_dna(network_seed: String) -> DnaFile {
        let (dna, _, _) = SweetDnaFile::from_zomes(
            network_seed,
            vec![TestIntegrityWasm::HcStressTestIntegrity],
            vec![TestCoordinatorWasm::HcStressTestCoordinator],
            vec![
                DnaWasm::from(TestIntegrityWasm::HcStressTestIntegrity),
                DnaWasm::from(TestCoordinatorWasm::HcStressTestCoordinator),
            ],
            SerializedBytes::default(),
        )
        .await;
        dna
    }

    /// Given a new/blank sweet conductor and the hc_stress_test dna
    /// (see [HcStressTest::test_dna]), install the dna, returning
    /// a conductor running the hc_stress_test app.
    pub async fn new(mut conductor: SweetConductor, dna: DnaFile) -> Self {
        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let mut cells = app.into_cells();

        Self {
            conductor,
            cell: cells.remove(0),
        }
    }

    /// Extract the file data from a Record.
    pub fn record_to_file_data(record: &Record) -> String {
        match record {
            Record {
                entry: RecordEntry::Present(Entry::App(AppEntryBytes(bytes))),
                ..
            } => {
                #[derive(Debug, serde::Deserialize)]
                struct F<'a> {
                    #[serde(with = "serde_bytes")]
                    data: &'a [u8],
                    #[allow(dead_code)]
                    uid: i64,
                }
                let f: F<'_> = decode(bytes.bytes()).unwrap();
                String::from_utf8_lossy(f.data).to_string()
            }
            _ => panic!("record does not contain file data"),
        }
    }

    /// Call the `create_file` zome function.
    pub async fn create_file(&mut self, data: &str) -> Record {
        #[derive(Debug, serde::Serialize)]
        struct F<'a> {
            #[serde(with = "serde_bytes")]
            data: &'a [u8],
            uid: i64,
        }
        self.conductor
            .call(
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "create_file",
                F {
                    data: data.as_bytes(),
                    uid: uid(),
                },
            )
            .await
    }

    /// Call the `get_all_images` zome function.
    pub async fn get_all_images(&mut self) -> Vec<ActionHash> {
        self.conductor
            .call(
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_all_images",
                (),
            )
            .await
    }

    /// Call the `get_file` zome function.
    pub async fn get_file(&mut self, hash: ActionHash) -> Record {
        self.conductor
            .call(
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_file",
                hash,
            )
            .await
    }
}
