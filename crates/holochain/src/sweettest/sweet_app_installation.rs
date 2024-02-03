use std::path::PathBuf;

use holochain_types::prelude::*;

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, clone limit of 255, and arbitrary role names
pub async fn app_bundle_from_dnas(dnas: impl IntoIterator<Item = &DnaFile>) -> AppBundle {
    let (roles, resources): (Vec<_>, Vec<_>) = dnas
        .into_iter()
        .map(|dna| {
            let path = PathBuf::from(format!("{}", dna.dna_hash()));
            let modifiers = DnaModifiersOpt::none();
            let installed_dna_hash = DnaHash::with_data_sync(dna.dna_def());
            let manifest = AppRoleManifest {
                name: dna.dna_hash().to_string(),
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
        .build()
        .unwrap();

    AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap()
}

/// Get a "standard" InstallAppPayload from a single DNA
pub async fn get_install_app_payload_from_dnas(
    installed_app_id: impl Into<InstalledAppId>,
    agent_key: Option<AgentPubKey>,
    dnas: impl IntoIterator<Item = &DnaFile>,
) -> InstallAppPayload {
    let bundle = app_bundle_from_dnas(dnas).await;
    InstallAppPayload {
        agent_key,
        source: AppBundleSource::Bundle(bundle),
        installed_app_id: Some(installed_app_id.into()),
        network_seed: None,
        membrane_proofs: Default::default(),
        #[cfg(feature = "chc")]
        ignore_genesis_failure: false,
    }
}
