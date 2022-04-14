use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasmPair;
use std::path::Path;

/// Helpful constructors for DnaFiles used in tests
#[derive(Clone, Debug, derive_more::From, derive_more::Into, shrinkwraprs::Shrinkwrap)]
pub struct SweetDnaFile(DnaFile);

impl SweetDnaFile {
    /// Create a DnaFile from a path to a *.dna bundle
    pub async fn from_bundle(path: &Path) -> DnaResult<DnaFile> {
        Self::from_bundle_with_overrides(path, None, Option::<()>::None).await
    }

    /// Create a DnaFile from a path to a *.dna bundle, applying the specified
    /// "phenotype" overrides
    pub async fn from_bundle_with_overrides<P>(
        path: &Path,
        uid: Option<Uid>,
        props: Option<P>,
    ) -> DnaResult<DnaFile>
    where
        P: Serialize,
    {
        let props = if let Some(p) = props {
            Some(YamlProperties::from(serde_yaml::to_value(p)?))
        } else {
            None
        };
        Ok(DnaBundle::read_from_file(path)
            .await?
            .into_dna_file(uid, props)
            .await?
            .0)
    }

    /// Create a DnaFile from a collection of Zomes
    pub async fn from_zomes(
        uid: String,
        integrity_zomes: IntegrityZomes,
        coordinator_zomes: CoordinatorZomes,
        wasms: Vec<wasm::DnaWasm>,
        properties: SerializedBytes,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)> {
        let dna_def = DnaDefBuilder::default()
            .uid(uid)
            .integrity_zomes(integrity_zomes.clone())
            .coordinator_zomes(coordinator_zomes.clone())
            .properties(properties.clone())
            .origin_time(Timestamp::HOLOCHAIN_EPOCH)
            .build()
            .unwrap();

        let dna_file = DnaFile::new(dna_def, wasms).await?;
        let integrity_zomes = integrity_zomes
            .into_iter()
            .map(|(n, z)| IntegrityZome::new(n, z))
            .collect();
        let coordinator_zomes = coordinator_zomes
            .into_iter()
            .map(|(n, z)| CoordinatorZome::new(n, z))
            .collect();
        Ok((dna_file, integrity_zomes, coordinator_zomes))
    }

    /// Create a DnaFile from a collection of Zomes,
    /// with a random UID
    pub async fn unique_from_zomes(
        integrity_zomes: IntegrityZomes,
        coordinator_zomes: CoordinatorZomes,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)> {
        Self::from_zomes(
            random_uid(),
            integrity_zomes,
            coordinator_zomes,
            wasms,
            SerializedBytes::default(),
        )
        .await
    }

    /// Create a DnaFile from a collection of TestWasm
    pub async fn from_test_wasms<W>(
        uid: String,
        wasms: Vec<W>,
        properties: SerializedBytes,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)>
    where
        W: Into<TestWasmPair<IntegrityZome, CoordinatorZome>>
            + Into<TestWasmPair<wasm::DnaWasm>>
            + Clone,
    {
        let (integrity_zomes, coordinator_zomes) = wasms
            .clone()
            .into_iter()
            .map(|w| {
                let TestWasmPair::<IntegrityZome, CoordinatorZome> {
                    integrity,
                    coordinator,
                } = w.into();
                (integrity.into_inner(), coordinator.into_inner())
            })
            .unzip();

        let wasms = wasms
            .into_iter()
            .flat_map(|w| {
                let TestWasmPair::<DnaWasm> {
                    integrity,
                    coordinator,
                } = w.into();
                [integrity, coordinator]
            })
            .collect();

        Self::from_zomes(uid, integrity_zomes, coordinator_zomes, wasms, properties).await
    }

    /// Create a DnaFile from a collection of TestWasm
    /// with a random UID
    pub async fn unique_from_test_wasms<W>(
        test_wasms: Vec<W>,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)>
    where
        W: Into<TestWasmPair<IntegrityZome, CoordinatorZome>>
            + Into<TestWasmPair<wasm::DnaWasm>>
            + Clone,
    {
        let (dna, integrity_zomes, coordinator_zomes) =
            Self::from_test_wasms(random_uid(), test_wasms, SerializedBytes::default()).await?;
        Ok((dna, integrity_zomes, coordinator_zomes))
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm)
    pub async fn from_inline_zomes(
        uid: String,
        zomes: InlineZomeSet,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)> {
        Self::from_zomes(
            uid,
            zomes
                .integrity_zomes
                .into_iter()
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            zomes
                .coordinator_zomes
                .into_iter()
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            Vec::with_capacity(0),
            SerializedBytes::default(),
        )
        .await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UID
    pub async fn unique_from_inline_zomes(
        zomes: InlineZomeSet,
    ) -> DnaResult<(DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)> {
        Self::from_inline_zomes(random_uid(), zomes).await
    }
}

/// Helpful constructors for DnaDefs used in tests
pub struct SweetDnaDef;

impl SweetDnaDef {
    /// Create a DnaDef with a random UID, useful for testing
    // TODO: move fully into sweettest when possible
    pub fn unique_from_zomes(
        integrity_zomes: Vec<IntegrityZome>,
        coordinator_zomes: Vec<CoordinatorZome>,
    ) -> DnaDef {
        DnaDef::unique_from_zomes(integrity_zomes, coordinator_zomes)
    }
}
