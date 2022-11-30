use holochain_p2p::dht::spacetime::STANDARD_QUANTUM_TIME;
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
        Self::from_bundle_with_overrides(path, DnaModifiersOpt::<SerializedBytes>::none()).await
    }

    /// Create a DnaFile from a path to a *.dna bundle, applying the specified
    /// modifier overrides
    pub async fn from_bundle_with_overrides<P, E>(
        path: &Path,
        modifiers: DnaModifiersOpt<P>,
    ) -> DnaResult<DnaFile>
    where
        P: TryInto<SerializedBytes, Error = E>,
        SerializedBytesError: From<E>,
    {
        Ok(DnaBundle::read_from_file(path)
            .await?
            .into_dna_file(modifiers.serialized().map_err(SerializedBytesError::from)?)
            .await?
            .0)
    }

    /// Create a DnaFile from a collection of Zomes
    pub async fn from_zomes<I, C, D>(
        network_seed: String,
        integrity_zomes: Vec<I>,
        coordinator_zomes: Vec<C>,
        wasms: Vec<D>,
        properties: SerializedBytes,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)
    where
        I: Into<IntegrityZome>,
        C: Into<CoordinatorZome>,
        D: Into<wasm::DnaWasm>,
    {
        let integrity_zomes: Vec<IntegrityZome> =
            integrity_zomes.into_iter().map(Into::into).collect();
        let coordinator_zomes: Vec<CoordinatorZome> =
            coordinator_zomes.into_iter().map(Into::into).collect();
        let iz: IntegrityZomes = integrity_zomes
            .clone()
            .into_iter()
            .map(IntegrityZome::into_inner)
            .collect();
        let cz: CoordinatorZomes = coordinator_zomes
            .clone()
            .into_iter()
            .map(CoordinatorZome::into_inner)
            .collect();
        let dna_def = DnaDefBuilder::default()
            .modifiers(DnaModifiers {
                network_seed,
                properties: properties.clone(),
                origin_time: Timestamp::HOLOCHAIN_EPOCH,
                quantum_time: STANDARD_QUANTUM_TIME,
            })
            .integrity_zomes(iz)
            .coordinator_zomes(cz)
            .build()
            .unwrap();

        let dna_file = DnaFile::new(dna_def, wasms.into_iter().map(Into::into)).await;
        (dna_file, integrity_zomes, coordinator_zomes)
    }

    /// Create a DnaFile from a collection of Zomes,
    /// with a random network seed
    pub async fn unique_from_zomes<I, C, D>(
        integrity_zomes: Vec<I>,
        coordinator_zomes: Vec<C>,
        wasms: Vec<D>,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)
    where
        I: Into<IntegrityZome>,
        C: Into<CoordinatorZome>,
        D: Into<wasm::DnaWasm>,
    {
        Self::from_zomes(
            random_network_seed(),
            integrity_zomes,
            coordinator_zomes,
            wasms,
            SerializedBytes::default(),
        )
        .await
    }

    /// Create a DnaFile from a collection of TestWasm
    pub async fn from_test_wasms<W>(
        network_seed: String,
        wasms: Vec<W>,
        properties: SerializedBytes,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)
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

        Self::from_zomes(
            network_seed,
            integrity_zomes,
            coordinator_zomes,
            wasms,
            properties,
        )
        .await
    }

    /// Create a DnaFile from a collection of TestWasm
    /// with a random network seed
    pub async fn unique_from_test_wasms<W>(
        test_wasms: Vec<W>,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>)
    where
        W: Into<TestWasmPair<IntegrityZome, CoordinatorZome>>
            + Into<TestWasmPair<wasm::DnaWasm>>
            + Clone,
    {
        let (dna, integrity_zomes, coordinator_zomes) = Self::from_test_wasms(
            random_network_seed(),
            test_wasms,
            SerializedBytes::default(),
        )
        .await;
        (dna, integrity_zomes, coordinator_zomes)
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm)
    pub async fn from_inline_zomes(
        network_seed: String,
        zomes: impl Into<InlineZomeSet>,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>) {
        let mut zomes = zomes.into();
        let coordinator_zomes: Vec<CoordinatorZome> = zomes
            .coordinator_zomes
            .into_iter()
            .map(|(n, z)| (n.into(), z.into()))
            .map(|t| {
                let dep = zomes.dependencies.remove(&t.0);
                let mut z: CoordinatorZome = t.into();
                if let Some(dep) = dep {
                    z.set_dependency(dep);
                }
                z
            })
            .collect();
        Self::from_zomes(
            network_seed,
            zomes
                .integrity_order
                .into_iter()
                .map(|n| zomes.integrity_zomes.remove_entry(n).unwrap())
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            coordinator_zomes,
            Vec::<wasm::DnaWasm>::with_capacity(0),
            SerializedBytes::default(),
        )
        .await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random network seed
    pub async fn unique_from_inline_zomes(
        zomes: impl Into<InlineZomeSet>,
    ) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>) {
        Self::from_inline_zomes(random_network_seed(), zomes).await
    }
}

/// Helpful constructors for DnaDefs used in tests
pub struct SweetDnaDef;

impl SweetDnaDef {
    /// Create a DnaDef with a random network seed, useful for testing
    // TODO: move fully into sweettest when possible
    pub fn unique_from_zomes(
        integrity_zomes: Vec<IntegrityZome>,
        coordinator_zomes: Vec<CoordinatorZome>,
    ) -> DnaDef {
        DnaDef::unique_from_zomes(integrity_zomes, coordinator_zomes)
    }
}
