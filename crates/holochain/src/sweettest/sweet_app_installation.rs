use std::path::PathBuf;

use holochain_types::prelude::*;

use crate::conductor::conductor::app_manifest_from_dnas;

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, clone limit of 255, and arbitrary role names
pub async fn app_bundle_from_dnas(
    dnas_with_roles: &[impl DnaWithRole],
    memproofs_deferred: bool,
) -> AppBundle {
    let (roles, resources): (Vec<_>, Vec<_>) = dnas_with_roles
        .iter()
        .map(|dr| {
            let dna = dr.dna();

            let path = PathBuf::from(format!("{}", dna.dna_hash()));
            let modifiers = DnaModifiersOpt::none();
            let manifest = AppRoleManifest {
                name: dr.role(),
                dna: AppRoleDnaManifest {
                    location: Some(DnaLocation::Bundled(path.clone())),
                    modifiers,
                    // NOTE: for testing with inline zomes, it's essential that the
                    //       installed_hash is included, so it can be used to fetch
                    //       the DNA file from the conductor's DNA store rather
                    //       than the one in the bundle which lacks inline zomes
                    //       due to serialization.
                    installed_hash: Some(dr.dna().dna_hash().clone().into()),
                    clone_limit: 255,
                },
                provisioning: Some(CellProvisioning::Create { deferred: false }),
            };
            let bundle = DnaBundle::from_dna_file(dna.clone()).unwrap();
            (manifest, (path, bundle))
        })
        .unzip();

    let manifest = AppManifestCurrentBuilder::default()
        .name("[generated]".into())
        .description(None)
        .roles(roles)
        .membrane_proofs_deferred(memproofs_deferred)
        .build()
        .unwrap()
        .into();

    debug_assert_eq!(
        manifest,
        app_manifest_from_dnas(dnas_with_roles, 255, memproofs_deferred),
        "app_bundle_from_dnas and app_manifest_from_dnas should produce the same manifest"
    );

    AppBundle::new(manifest, resources, PathBuf::from("."))
        .await
        .unwrap()
}

/// Get a "standard" InstallAppPayload from a single DNA
pub async fn get_install_app_payload_from_dnas(
    installed_app_id: impl Into<InstalledAppId>,
    agent_key: Option<AgentPubKey>,
    data: &[(impl DnaWithRole, Option<MembraneProof>)],
) -> InstallAppPayload {
    let dnas_with_roles: Vec<_> = data.iter().map(|(dr, _)| dr).cloned().collect();
    let bundle = app_bundle_from_dnas(&dnas_with_roles, false).await;
    let membrane_proofs = data
        .iter()
        .map(|(dr, memproof)| (dr.role(), memproof.clone().unwrap_or_default()))
        .collect();

    InstallAppPayload {
        agent_key,
        source: AppBundleSource::Bundle(bundle),
        installed_app_id: Some(installed_app_id.into()),
        network_seed: None,
        membrane_proofs,
        #[cfg(feature = "chc")]
        ignore_genesis_failure: false,
    }
}
