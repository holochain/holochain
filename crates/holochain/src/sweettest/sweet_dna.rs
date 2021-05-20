use holochain_types::{
    dna::{
        error::DnaResult,
        random_uid, wasm,
        zome::{inline_zome::InlineZome, Zome, ZomeDef},
        DnaDefBuilder, DnaFile,
    },
    prelude::{DnaBundle, DnaDef},
};
use holochain_zome_types::zome::ZomeName;
use std::path::Path;

/// Helpful constructors for DnaFiles used in tests
#[derive(Clone, Debug, derive_more::From, derive_more::Into, shrinkwraprs::Shrinkwrap)]
pub struct SweetDnaFile(DnaFile);

impl SweetDnaFile {
    /// Create a DnaFile from a path to a *.dna bundle
    pub async fn from_bundle(path: &Path) -> DnaResult<DnaFile> {
        Ok(DnaBundle::read_from_file(path)
            .await?
            .into_dna_file(None, None)
            .await?
            .0)
    }

    /// Create a DnaFile from a collection of Zomes
    pub async fn from_zomes(
        uid: String,
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        let dna_def = DnaDefBuilder::default()
            .uid(uid)
            .zomes(zomes.clone())
            .build()
            .unwrap();

        let dna_file = DnaFile::new(dna_def, wasms).await?;
        let zomes: Vec<Zome> = zomes.into_iter().map(|(n, z)| Zome::new(n, z)).collect();
        Ok((dna_file, zomes))
    }

    /// Create a DnaFile from a collection of Zomes,
    /// with a random UID
    pub async fn unique_from_zomes(
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_zomes(random_uid(), zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    pub async fn from_test_wasms<W>(
        uid: String,
        test_wasms: Vec<W>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        let zomes = test_wasms.clone().into_iter().map(Into::into).collect();
        let wasms = test_wasms.into_iter().map(Into::into).collect();
        Self::from_zomes(uid, zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    /// with a random UID
    pub async fn unique_from_test_wasms<W>(test_wasms: Vec<W>) -> DnaResult<(DnaFile, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        Self::from_test_wasms(random_uid(), test_wasms).await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm)
    pub async fn from_inline_zomes(
        uid: String,
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_zomes(
            uid,
            zomes
                .into_iter()
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            Vec::new(),
        )
        .await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UID
    pub async fn unique_from_inline_zomes(
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_inline_zomes(random_uid(), zomes).await
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    pub async fn from_inline_zome(
        uid: String,
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(DnaFile, Zome)> {
        let (dna_file, mut zomes) = Self::from_inline_zomes(uid, vec![(zome_name, zome)]).await?;
        Ok((dna_file, zomes.pop().unwrap()))
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    /// with a random UID
    pub async fn unique_from_inline_zome(
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(DnaFile, Zome)> {
        Self::from_inline_zome(random_uid(), zome_name, zome).await
    }
}

/// Helpful constructors for DnaDefs used in tests
pub struct SweetDnaDef;

impl SweetDnaDef {
    /// Create a DnaDef with a random UID, useful for testing
    // TODO: move fully into sweettest when possible
    pub fn unique_from_zomes(zomes: Vec<Zome>) -> DnaDef {
        DnaDef::unique_from_zomes(zomes)
    }
}
