use crate::conductor::conductor::app_manifest_from_dnas;
use holochain_types::prelude::*;

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, clone limit of 255, and arbitrary role names
pub async fn app_bundle_from_dnas(
    dnas_with_roles: &[impl DnaWithRole],
    memproofs_deferred: bool,
    network_seed: Option<NetworkSeed>,
) -> AppBundle {
    let (roles, resources): (Vec<_>, Vec<_>) = dnas_with_roles
        .iter()
        .map(|dr| {
            let dna = dr.dna();

            let modifiers = DnaModifiersOpt::none();
            let path = format!("{}", dna.dna_hash());
            let manifest = AppRoleManifest {
                name: dr.role(),
                dna: AppRoleDnaManifest {
                    path: Some(path.clone()),
                    modifiers,
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
        .allow_deferred_memproofs(memproofs_deferred)
        .build()
        .unwrap()
        .into();

    debug_assert_eq!(
        manifest,
        app_manifest_from_dnas(dnas_with_roles, 255, memproofs_deferred, network_seed),
        "app_bundle_from_dnas and app_manifest_from_dnas should produce the same manifest"
    );

    AppBundle::new(manifest, resources).unwrap()
}

/// Get a "standard" InstallAppPayload from a single DNA
pub async fn get_install_app_payload_from_dnas(
    installed_app_id: impl Into<InstalledAppId>,
    agent_key: Option<AgentPubKey>,
    data: &[(impl DnaWithRole, Option<MembraneProof>)],
    network_seed: Option<NetworkSeed>,
) -> InstallAppPayload {
    let dnas_with_roles: Vec<_> = data.iter().map(|(dr, _)| dr).cloned().collect();
    let bundle = app_bundle_from_dnas(&dnas_with_roles, false, network_seed).await;
    let roles_settings = Some(
        data.iter()
            .map(|(dr, memproof)| {
                (
                    dr.role(),
                    RoleSettings::Provisioned {
                        modifiers: Default::default(),
                        membrane_proof: memproof.clone(),
                    },
                )
            })
            .collect(),
    );

    let bytes = bundle.pack().expect("failed to encode app bundle as bytes");
    InstallAppPayload {
        agent_key,
        source: AppBundleSource::Bytes(bytes),
        installed_app_id: Some(installed_app_id.into()),
        network_seed: None,
        roles_settings,
        ignore_genesis_failure: false,
    }
}
