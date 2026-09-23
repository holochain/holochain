//! Submit membrane proofs to apps that defer proof-dependent genesis.

use anyhow::Context;
use clap::{ArgGroup, Args};
use holochain_client::AdminWebsocket;
use holochain_types::app::OpaqueBytesSource;
use holochain_types::prelude::{AppStatus, MemproofMap, RoleName};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Arguments for supplying membrane proofs to a deferred application.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("proof_input")
        .required(true)
        .args(["proofs", "proofs_file"])
))]
pub struct ProvideMemproofs {
    /// Connection options for selecting a target conductor.
    #[command(flatten)]
    pub connect_args: crate::zome_call::ConnectArgs,

    /// The installed app id awaiting membrane proofs.
    pub app_id: String,

    /// Optional origin header for the admin websocket connection.
    #[arg(long)]
    pub origin: Option<String>,

    /// Read raw membrane-proof bytes from ROLE=PATH; may be repeated for distinct roles.
    #[arg(
        long = "membrane-proof",
        value_name = "ROLE=PATH",
        value_parser = crate::calls::parse_membrane_proof
    )]
    pub proofs: Vec<(String, PathBuf)>,

    /// Read a direct role-to-byte-source YAML map.
    #[arg(long = "membrane-proofs", value_name = "YAML")]
    pub proofs_file: Option<PathBuf>,
}

/// Load proof bytes from the selected command-line input.
fn load_memproofs(args: &ProvideMemproofs) -> anyhow::Result<MemproofMap> {
    if let Some(path) = &args.proofs_file {
        let yaml = fs::read_to_string(path)
            .with_context(|| format!("failed to read membrane proofs YAML {}", path.display()))?;
        let sources: HashMap<RoleName, OpaqueBytesSource> = yaml_serde::from_str(&yaml)
            .with_context(|| format!("failed to parse membrane proofs YAML {}", path.display()))?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        return sources
            .into_iter()
            .map(|(role, source)| {
                let bytes = source
                    .resolve(base_dir)
                    .with_context(|| format!("failed to resolve membrane_proof for role {role}"))?;
                Ok((role, Arc::new(bytes)))
            })
            .collect::<anyhow::Result<MemproofMap>>()
            .with_context(|| {
                format!(
                    "failed to resolve membrane proofs YAML {path}",
                    path = path.display()
                )
            });
    }

    let mut roles = HashSet::new();
    for (role, _) in &args.proofs {
        anyhow::ensure!(roles.insert(role), "duplicate membrane proof role: {role}");
    }
    let base_dir = std::env::current_dir().context("failed to determine current directory")?;
    args.proofs
        .iter()
        .map(|(role, path)| {
            let bytes = OpaqueBytesSource::Path { path: path.clone() }
                .resolve(&base_dir)
                .with_context(|| format!("failed to resolve membrane_proof for role {role}"))?;
            Ok((role.clone(), Arc::new(bytes)))
        })
        .collect()
}

/// Provide membrane proofs to an app and print its updated app information.
pub async fn provide_memproofs(args: ProvideMemproofs) -> anyhow::Result<()> {
    let memproofs = load_memproofs(&args)?;
    let admin = AdminWebsocket::connect(
        format!("localhost:{}", args.connect_args.port),
        args.origin.clone(),
    )
    .await?;
    let info = admin
        .list_apps(None)
        .await?
        .into_iter()
        .find(|info| info.installed_app_id == args.app_id)
        .ok_or_else(|| anyhow::anyhow!("app not found: {}", args.app_id))?;
    anyhow::ensure!(
        matches!(info.status, AppStatus::AwaitingMemproofs),
        "app {} is not awaiting membrane proofs",
        args.app_id
    );
    let roles = info.manifest.app_roles();
    for role in memproofs.keys() {
        anyhow::ensure!(
            roles.iter().any(|candidate| &candidate.name == role),
            "unknown membrane proof role: {role}"
        );
    }

    let app = crate::zome_call::get_app_client(&admin, args.app_id.clone(), None).await?;
    app.provide_memproofs(memproofs)
        .await
        .context("failed to provide membrane proofs")?;
    let updated = app
        .app_info()
        .await
        .context("proofs accepted but app info failed; check hc client call list-apps")?
        .context("proofs accepted but app disappeared; check hc client call list-apps")?;
    println!(
        "{}",
        serde_json::to_string(&crate::calls::app_info_to_base64_json(updated)?)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HcClient;
    use clap::Parser;
    use tempfile::TempDir;

    fn args_with_file(path: PathBuf) -> ProvideMemproofs {
        ProvideMemproofs {
            connect_args: crate::zome_call::ConnectArgs { port: 12345 },
            app_id: "app".to_string(),
            origin: None,
            proofs: Vec::new(),
            proofs_file: Some(path),
        }
    }

    #[test]
    fn deferred_membrane_proofs_loads_all_supported_sources() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("proof.bin");
        std::fs::write(&binary_path, [0x00, 0xff, 0x80]).unwrap();
        let yaml_path = temp_dir.path().join("proofs.yaml");
        std::fs::write(
            &yaml_path,
            "base64-role:\n  base64: AP+A\npath-role:\n  path: proof.bin\ntext-role: literal\narray-role: [0, 255, 128]\n",
        )
        .unwrap();

        let loaded = load_memproofs(&args_with_file(yaml_path)).unwrap();
        assert_eq!(loaded["base64-role"].bytes(), &[0x00, 0xff, 0x80]);
        assert_eq!(loaded["path-role"].bytes(), &[0x00, 0xff, 0x80]);
        assert_eq!(loaded["text-role"].bytes(), b"literal");
        assert_eq!(loaded["array-role"].bytes(), &[0x00, 0xff, 0x80]);
    }

    #[test]
    fn deferred_membrane_proofs_accepts_empty_yaml_map() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("empty.yaml");
        std::fs::write(&yaml_path, "{}\n").unwrap();
        assert!(load_memproofs(&args_with_file(yaml_path))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn deferred_membrane_proofs_missing_file_reports_yaml_context() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("proofs.yaml");
        std::fs::write(&yaml_path, "role-1:\n  path: missing.bin\n").unwrap();
        let error = load_memproofs(&args_with_file(yaml_path.clone())).unwrap_err();
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains(&yaml_path.display().to_string()),
            "{rendered}"
        );
        assert!(rendered.contains("role-1"));
        assert!(rendered.contains("membrane_proof"));
        assert!(rendered.contains(&temp_dir.path().join("missing.bin").display().to_string()));
        assert!(matches!(
            error.downcast_ref::<holochain_types::app::OpaqueBytesSourceError>(),
            Some(holochain_types::app::OpaqueBytesSourceError::Read { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[test]
    fn deferred_membrane_proofs_invalid_base64_reports_yaml_context() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("proofs.yaml");
        std::fs::write(&yaml_path, "role-1:\n  base64: invalid base64 !!!\n").unwrap();
        let error = load_memproofs(&args_with_file(yaml_path.clone())).unwrap_err();
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains(&yaml_path.display().to_string()),
            "{rendered}"
        );
        assert!(rendered.contains("role-1"));
        assert!(rendered.contains("membrane_proof"));
        assert!(matches!(
            error.downcast_ref::<holochain_types::app::OpaqueBytesSourceError>(),
            Some(holochain_types::app::OpaqueBytesSourceError::Base64(_))
        ));
        assert!(!rendered.contains("invalid base64 !!!"));
    }

    #[test]
    fn deferred_membrane_proofs_rejects_roles_settings_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("roles.yaml");
        std::fs::write(
            &yaml_path,
            "role-1:\n  type: provisioned\n  modifiers: {}\n",
        )
        .unwrap();
        assert!(load_memproofs(&args_with_file(yaml_path)).is_err());
    }

    #[test]
    fn deferred_membrane_proofs_rejects_duplicate_flags() {
        let args = ProvideMemproofs {
            connect_args: crate::zome_call::ConnectArgs { port: 12345 },
            app_id: "app".to_string(),
            origin: None,
            proofs: vec![
                ("role-1".to_string(), PathBuf::from("one.bin")),
                ("role-1".to_string(), PathBuf::from("two.bin")),
            ],
            proofs_file: None,
        };
        let error = load_memproofs(&args).unwrap_err();
        assert!(error.to_string().contains("duplicate"));
    }

    #[test]
    fn deferred_membrane_proofs_parser_checks_input_modes() {
        let repeated = HcClient::try_parse_from([
            "hc-client",
            "provide-memproofs",
            "--port",
            "12345",
            "app",
            "--membrane-proof",
            "role-1=one.bin",
            "--membrane-proof",
            "role-2=two.bin",
        ]);
        assert!(repeated.is_ok(), "{repeated:?}");

        let missing =
            HcClient::try_parse_from(["hc-client", "provide-memproofs", "--port", "12345", "app"]);
        assert!(missing.is_err());

        let incompatible = HcClient::try_parse_from([
            "hc-client",
            "provide-memproofs",
            "--port",
            "12345",
            "app",
            "--membrane-proof",
            "role-1=one.bin",
            "--membrane-proofs",
            "proofs.yaml",
        ]);
        assert!(incompatible.is_err());

        let malformed = HcClient::try_parse_from([
            "hc-client",
            "provide-memproofs",
            "--port",
            "12345",
            "app",
            "--membrane-proof",
            "role-without-path",
        ]);
        assert!(malformed.is_err());
    }
}
