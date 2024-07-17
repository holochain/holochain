use std::path::PathBuf;

use holochain_types::prelude::*;

use super::DnaWithRole;

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, clone limit of 255, and arbitrary role names
pub async fn app_bundle_from_dnas<'a>(
    dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)>,
    memproofs_deferred: bool,
) -> AppBundle {
    let (roles, resources): (Vec<_>, Vec<_>) = dnas_with_roles
        .into_iter()
        .map(|dr| {
            let dna = dr.dna();

            let path = PathBuf::from(format!("{}", dna.dna_hash()));
            let modifiers = DnaModifiersOpt::none();
            let installed_dna_hash = DnaHash::with_data_sync(dna.dna_def());
            let manifest = AppRoleManifest {
                name: dr.role(),
                dna: AppRoleDnaManifest {
                    location: Some(DnaLocation::Bundled(path.clone())),
                    modifiers,
                    installed_hash: Some(installed_dna_hash.into()),
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
        .unwrap();

    AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap()
}

/// Get a "standard" InstallAppPayload from a single DNA
pub async fn get_install_app_payload_from_dnas(
    installed_app_id: impl Into<InstalledAppId>,
    agent_key: AgentPubKey,
    data: &[(impl DnaWithRole, Option<MembraneProof>)],
) -> InstallAppPayload {
    let dnas_with_roles: Vec<_> = data.iter().map(|(dr, _)| dr).collect();
    let bundle = app_bundle_from_dnas(dnas_with_roles, false).await;
    let membrane_proofs = Some(
        data.iter()
            .map(|(dr, memproof)| (dr.role(), memproof.clone().unwrap_or_default()))
            .collect(),
    );

    InstallAppPayload {
        agent_key,
        source: AppBundleSource::Bundle(bundle),
        installed_app_id: Some(installed_app_id.into()),
        network_seed: None,
        membrane_proofs,
        ignore_genesis_failure: false,
    }
}
