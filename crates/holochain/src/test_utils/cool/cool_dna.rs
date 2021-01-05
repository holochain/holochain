use holochain_types::dna::{
    error::DnaResult,
    wasm,
    zome::{inline_zome::InlineZome, Zome, ZomeDef},
    DnaDefBuilder, DnaFile,
};
use holochain_zome_types::zome::ZomeName;

/// Helpful constructors for DnaFiles used in tests
#[derive(Clone, Debug, derive_more::From, derive_more::Into, shrinkwraprs::Shrinkwrap)]
pub struct CoolDnaFile(DnaFile);

impl CoolDnaFile {
    /// Create a DnaFile from a collection of Zomes
    pub async fn from_zomes(
        uuid: String,
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        let dna_def = DnaDefBuilder::default()
            .uuid(uuid)
            .zomes(zomes.clone())
            .build()
            .unwrap();

        let dna_file = DnaFile::new(dna_def, wasms).await?;
        let zomes: Vec<Zome> = zomes.into_iter().map(|(n, z)| Zome::new(n, z)).collect();
        Ok((dna_file, zomes))
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UUID
    pub async fn unique_from_zomes(
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_zomes(random_uuid(), zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    pub async fn from_test_wasms<W>(
        uuid: String,
        test_wasms: Vec<W>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        let zomes = test_wasms.clone().into_iter().map(Into::into).collect();
        let wasms = test_wasms.into_iter().map(Into::into).collect();
        Self::from_zomes(uuid, zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    /// with a random UUID
    pub async fn unique_from_test_wasms<W>(test_wasms: Vec<W>) -> DnaResult<(DnaFile, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        Self::from_test_wasms(random_uuid(), test_wasms).await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm)
    pub async fn from_inline_zomes(
        uuid: String,
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_zomes(
            uuid,
            zomes
                .into_iter()
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            Vec::new(),
        )
        .await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UUID
    pub async fn unique_from_inline_zomes(
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(DnaFile, Vec<Zome>)> {
        Self::from_inline_zomes(random_uuid(), zomes).await
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    pub async fn from_inline_zome(
        uuid: String,
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(DnaFile, Zome)> {
        let (dna_file, mut zomes) = Self::from_inline_zomes(uuid, vec![(zome_name, zome)]).await?;
        Ok((dna_file, zomes.pop().unwrap()))
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    /// with a random UUID
    pub async fn unique_from_inline_zome(
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(DnaFile, Zome)> {
        Self::from_inline_zome(random_uuid(), zome_name, zome).await
    }
}

fn random_uuid() -> String {
    nanoid::nanoid!()
}
