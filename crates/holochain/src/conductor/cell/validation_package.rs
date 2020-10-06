use holochain_state::{env::EnvironmentRead, error::DatabaseResult, prelude::*};
use holochain_types::dna::DnaFile;
use holochain_zome_types::Header;

use crate::core::state::cascade::{Cascade, DbPair, DbPairMut};

use super::*;

/// Databases to search for validation package
pub(super) struct ValidationPackageDb {
    element_integrated: ElementBuf<IntegratedPrefix>,
    meta_integrated: MetadataBuf<IntegratedPrefix>,
    element_rejected: ElementBuf<RejectedPrefix>,
    meta_rejected: MetadataBuf<RejectedPrefix>,
    // TODO Change to authored when #394 is merged in
    element_cache: ElementBuf,
    meta_cache: MetadataBuf,
}

impl ValidationPackageDb {
    pub(super) fn create(env: EnvironmentRead) -> DatabaseResult<Self> {
        Ok(Self {
            element_integrated: ElementBuf::vault(env.clone(), false)?,
            element_rejected: ElementBuf::rejected(env.clone())?,
            element_cache: ElementBuf::cache(env.clone())?,
            meta_integrated: MetadataBuf::vault(env.clone())?,
            meta_rejected: MetadataBuf::rejected(env.clone())?,
            meta_cache: MetadataBuf::cache(env)?,
        })
    }

    // TODO: remove mut when #394 lands
    pub(super) fn cascade(&mut self) -> Cascade {
        Cascade::empty()
            .with_integrated(DbPair::new(&self.element_integrated, &self.meta_integrated))
            .with_rejected(DbPair::new(&self.element_rejected, &self.meta_rejected))
            // TODO Change to authored when #394 is merged in
            .with_cache(DbPairMut::new(
                &mut self.element_cache,
                &mut self.meta_cache,
            ))
    }
}

pub(super) async fn get_as_author(
    header: Header,
    env: EnvironmentRead,
    dna_file: &DnaFile,
    conductor_api: &impl CellConductorApiT,
) -> CellResult<ValidationPackageResponse> {
    // Get the source chain with public data only
    let source_chain = SourceChain::public_only(env)?;

    // Get the header data
    let (app_entry_type, header_seq) = match header
        .entry_type()
        .cloned()
        .map(|et| (et, header.header_seq()))
    {
        Some((EntryType::App(aet), header_seq)) => (aet, header_seq),
        _ => return Ok(None.into()),
    };

    //Get entry def
    let entry_def = get_entry_def_from_ids(
        app_entry_type.zome_id(),
        app_entry_type.id(),
        dna_file,
        conductor_api,
    )
    .await?;

    // Get the required validation package
    let required_validation_package = match entry_def {
        Some(ed) => ed.required_validation_package,
        None => return Ok(None.into()),
    };

    // Gather the package
    match required_validation_package {
        RequiredValidationPackage::Element => {
            // TODO: I'm not sure if we should handle this case, it seems like they should already have the element
            Ok(None.into())
        }
        RequiredValidationPackage::SubChain => {
            // Collect and return the sub chain
            let elements = source_chain.query(
                &ChainQueryFilter::default()
                    .include_entries(true)
                    .entry_type(EntryType::App(app_entry_type))
                    // Range is exclusive but we want to include this header
                    .sequence_range(0..header_seq + 1),
            )?;
            Ok(Some(ValidationPackage::new(elements)).into())
        }
        RequiredValidationPackage::Full => {
            let elements = source_chain.query(
                &ChainQueryFilter::default()
                    .include_entries(true)
                    .sequence_range(0..header_seq + 1),
            )?;
            Ok(Some(ValidationPackage::new(elements)).into())
        }
    }
}
