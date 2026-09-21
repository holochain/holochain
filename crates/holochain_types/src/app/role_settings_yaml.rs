//! YAML sources for role-specific application installation settings.
//!
//! This module keeps YAML deserialization free of filesystem access. Explicit
//! base64 and path sources are resolved only when a caller loads settings for
//! installation, while legacy strings and integer arrays retain their original
//! byte meanings.

use super::{RoleSettings, RoleSettingsMap};
use base64::Engine;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_zome_types::prelude::{
    CellId, DnaModifiersOpt, InitProperties, RoleName, YamlProperties,
};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A YAML source for opaque application-defined bytes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum OpaqueBytesSource {
    /// Decode the source value as standard base64.
    Base64 {
        /// The standard base64 representation of the bytes.
        base64: String,
    },
    /// Read the bytes from a file when the source is resolved.
    Path {
        /// The path to resolve relative to the settings file or caller.
        path: PathBuf,
    },
    /// Preserve a legacy YAML byte array exactly.
    Bytes(Vec<u8>),
    /// Preserve a legacy YAML string as its UTF-8 bytes.
    Text(String),
}

impl OpaqueBytesSource {
    /// Resolves this source into the exact bytes it describes.
    ///
    /// Relative paths are joined to `base_dir`; absolute paths are unchanged.
    /// This method performs file I/O only for [`Self::Path`].
    ///
    /// # Errors
    ///
    /// Returns an error when base64 decoding fails or a path cannot be read.
    pub fn resolve(self, base_dir: &Path) -> Result<SerializedBytes, OpaqueBytesSourceError> {
        let bytes = match self {
            Self::Base64 { base64 } => base64::engine::general_purpose::STANDARD
                .decode(base64)
                .map_err(OpaqueBytesSourceError::Base64)?,
            Self::Path { path } => {
                let path = base_dir.join(path);
                std::fs::read(&path)
                    .map_err(|source| OpaqueBytesSourceError::Read { path, source })?
            }
            Self::Bytes(bytes) => bytes,
            Self::Text(text) => text.into_bytes(),
        };
        Ok(SerializedBytes::from(UnsafeBytes::from(bytes)))
    }
}

/// Errors encountered while resolving one opaque byte source.
#[derive(Debug, thiserror::Error)]
pub enum OpaqueBytesSourceError {
    /// The source was not valid standard base64.
    #[error("failed to decode opaque bytes as standard base64")]
    Base64(#[source] base64::DecodeError),
    /// The source file could not be read.
    #[error("failed to read opaque bytes from {path}")]
    Read {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying read error.
        #[source]
        source: io::Error,
    },
}

/// Errors encountered while reading or resolving YAML role settings.
#[derive(Debug, thiserror::Error)]
pub enum RoleSettingsYamlError {
    /// The settings file could not be read as UTF-8 text.
    #[error("failed to read role settings file {path}")]
    Read {
        /// The settings file path.
        path: PathBuf,
        /// The underlying read error.
        #[source]
        source: io::Error,
    },
    /// The settings file could not be parsed as YAML.
    #[error("failed to parse role settings file {path}")]
    Parse {
        /// The settings file path.
        path: PathBuf,
        /// The underlying YAML parse error.
        #[source]
        source: yaml_serde::Error,
    },
    /// A bytes field could not be resolved.
    #[error("failed to resolve role settings field `{field}`")]
    Field {
        /// The failing field name.
        field: &'static str,
        /// The underlying source resolution error.
        #[source]
        source: OpaqueBytesSourceError,
    },
    /// More than one flag supplied a proof for the same role.
    #[error("duplicate membrane proof override for role `{role}`")]
    DuplicateProof {
        /// The duplicated role name.
        role: RoleName,
    },
    /// A flag attempted to replace a proof already supplied by YAML.
    #[error("membrane proof already supplied for role `{role}`")]
    ProofConflict {
        /// The conflicting role name.
        role: RoleName,
    },
    /// A flag attempted to configure a role that reuses an existing cell.
    #[error("membrane proof cannot be supplied for existing-cell role `{role}`")]
    ExistingCell {
        /// The existing-cell role name.
        role: RoleName,
    },
    /// The invocation directory could not be obtained for a relative flag path.
    #[error("failed to resolve relative membrane proof paths from the current directory")]
    CurrentDirectory(#[source] io::Error),
    /// A role's settings could not be resolved.
    #[error("failed to resolve role `{role}`")]
    Role {
        /// The failing role name.
        role: RoleName,
        /// The role-specific error.
        #[source]
        source: Box<Self>,
    },
    /// A role-resolution error associated with a settings file.
    #[error("failed to resolve role settings file {path}")]
    Settings {
        /// The settings file path.
        path: PathBuf,
        /// The underlying role-resolution error.
        #[source]
        source: Box<Self>,
    },
}

/// YAML role settings with sources that have not yet been resolved.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoleSettingsYaml {
    #[deprecated(
        since = "0.6.0-dev.17",
        note = "For late binding, update the coordinators of a DNA. For calling cells of other apps, use bridge calls."
    )]
    /// Reuse an existing cell assigned to this role.
    UseExisting {
        /// The existing cell identifier.
        cell_id: CellId,
    },
    /// Optional settings for a normally provisioned cell.
    Provisioned {
        /// Optional membrane-proof bytes for the role.
        membrane_proof: Option<OpaqueBytesSource>,
        /// Optional DNA modifier overrides for the role.
        modifiers: Option<DnaModifiersOpt<YamlProperties>>,
        /// Optional opaque bytes made available during `init`.
        init_properties: Option<OpaqueBytesSource>,
    },
}

impl RoleSettingsYaml {
    /// Resolves YAML sources into conductor installation settings.
    ///
    /// # Errors
    ///
    /// Returns a field-specific error when a membrane proof or init property
    /// cannot be decoded or read.
    pub fn resolve(self, base_dir: &Path) -> Result<RoleSettings, RoleSettingsYamlError> {
        match self {
            Self::Provisioned {
                membrane_proof,
                modifiers,
                init_properties,
            } => {
                let membrane_proof = membrane_proof
                    .map(|source| source.resolve(base_dir).map(Arc::new))
                    .transpose()
                    .map_err(|source| RoleSettingsYamlError::Field {
                        field: "membrane_proof",
                        source,
                    })?;
                let init_properties = init_properties
                    .map(|source| source.resolve(base_dir).map(InitProperties))
                    .transpose()
                    .map_err(|source| RoleSettingsYamlError::Field {
                        field: "init_properties",
                        source,
                    })?;
                Ok(RoleSettings::Provisioned {
                    membrane_proof,
                    modifiers,
                    init_properties,
                })
            }
            #[expect(deprecated, reason = "preserve the existing UseExisting YAML setting")]
            Self::UseExisting { cell_id } => Ok(RoleSettings::UseExisting { cell_id }),
        }
    }

    /// Returns the YAML modifier overrides without resolving byte sources.
    pub fn modifiers(&self) -> Option<&DnaModifiersOpt<YamlProperties>> {
        match self {
            #[expect(deprecated, reason = "preserve the existing UseExisting YAML setting")]
            Self::UseExisting { .. } => None,
            Self::Provisioned { modifiers, .. } => modifiers.as_ref(),
        }
    }
}

/// Reads and resolves a YAML role-settings file.
///
/// Relative byte-source paths are resolved relative to the settings file's
/// parent directory. Deserialization itself never reads source files.
///
/// # Errors
///
/// Returns contextual errors for file reads, YAML parsing, and byte-source
/// resolution.
pub fn read_role_settings_yaml(path: &Path) -> Result<RoleSettingsMap, RoleSettingsYamlError> {
    let settings = read_sources(path)?;
    let base_dir = path.parent().unwrap_or(Path::new("."));
    resolve_sources(settings, base_dir).map_err(|source| RoleSettingsYamlError::Settings {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Reads YAML role settings and applies client membrane-proof file overrides.
///
/// Relative flag paths are resolved against the process invocation directory,
/// while YAML paths remain relative to the YAML file. The process directory is
/// never changed.
///
/// # Errors
///
/// Returns errors for duplicate flags, incompatible role settings, current
/// directory lookup failures, source reads, parsing, and byte resolution.
#[expect(deprecated, reason = "preserve existing-cell role settings")]
pub fn read_role_settings_yaml_with_proofs(
    settings_path: Option<&Path>,
    proofs: &[(RoleName, PathBuf)],
) -> Result<Option<RoleSettingsMap>, RoleSettingsYamlError> {
    if settings_path.is_none() && proofs.is_empty() {
        return Ok(None);
    }

    let mut settings = settings_path
        .map(read_sources)
        .transpose()?
        .unwrap_or_default();

    let mut roles = HashSet::with_capacity(proofs.len());
    for (role, _) in proofs {
        if !roles.insert(role.clone()) {
            return Err(RoleSettingsYamlError::DuplicateProof { role: role.clone() });
        }
        match settings.get(role) {
            Some(RoleSettingsYaml::Provisioned {
                membrane_proof: Some(_),
                ..
            }) => {
                return Err(RoleSettingsYamlError::ProofConflict { role: role.clone() });
            }
            Some(RoleSettingsYaml::UseExisting { .. }) => {
                return Err(RoleSettingsYamlError::ExistingCell { role: role.clone() });
            }
            Some(RoleSettingsYaml::Provisioned {
                membrane_proof: None,
                ..
            })
            | None => {}
        }
    }

    let current_dir = if proofs.iter().any(|(_, path)| path.is_relative()) {
        Some(std::env::current_dir().map_err(RoleSettingsYamlError::CurrentDirectory)?)
    } else {
        None
    };
    for (role, path) in proofs {
        let path = current_dir
            .as_deref()
            .map_or_else(|| path.clone(), |current_dir| current_dir.join(path));
        let settings =
            settings
                .entry(role.clone())
                .or_insert_with(|| RoleSettingsYaml::Provisioned {
                    membrane_proof: None,
                    modifiers: None,
                    init_properties: None,
                });
        match settings {
            RoleSettingsYaml::Provisioned { membrane_proof, .. } => {
                *membrane_proof = Some(OpaqueBytesSource::Path { path })
            }
            RoleSettingsYaml::UseExisting { .. } => {
                return Err(RoleSettingsYamlError::ExistingCell { role: role.clone() });
            }
        }
    }

    let base_dir = settings_path
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));
    let resolved = resolve_sources(settings, base_dir).map_err(|source| match settings_path {
        Some(path) => RoleSettingsYamlError::Settings {
            path: path.to_path_buf(),
            source: Box::new(source),
        },
        None => source,
    })?;
    Ok(Some(resolved))
}

fn read_sources(path: &Path) -> Result<RoleSettingsMapYaml, RoleSettingsYamlError> {
    let yaml = std::fs::read_to_string(path).map_err(|source| RoleSettingsYamlError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    yaml_serde::from_str(&yaml).map_err(|source| RoleSettingsYamlError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_sources(
    settings: RoleSettingsMapYaml,
    base_dir: &Path,
) -> Result<RoleSettingsMap, RoleSettingsYamlError> {
    settings
        .into_iter()
        .map(|(role, settings)| {
            settings
                .resolve(base_dir)
                .map(|settings| (role.clone(), settings))
                .map_err(|source| RoleSettingsYamlError::Role {
                    role,
                    source: Box::new(source),
                })
        })
        .collect()
}

/// The YAML map accepted by role-settings installation callers.
pub type RoleSettingsMapYaml = std::collections::HashMap<RoleName, RoleSettingsYaml>;

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::*;
    use holo_hash::{AgentPubKey, DnaHash};

    #[test]
    fn roles_settings_resolve_exact_bytes_relative_to_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let proof = vec![0, 255, 128, 10];
        std::fs::write(dir.path().join("proof.bin"), &proof).unwrap();
        let yaml = dir.path().join("roles.yaml");
        std::fs::write(
            &yaml,
            concat!(
                "role-1:\n  type: provisioned\n  membrane_proof:\n",
                "    path: proof.bin\n  init_properties:\n    base64: AQID\n",
            ),
        )
        .unwrap();
        let mut roles = read_role_settings_yaml(&yaml).unwrap();
        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = roles.remove("role-1").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.unwrap().bytes(), &proof);
        assert_eq!(init_properties.unwrap().0.bytes(), &vec![1, 2, 3]);
    }

    #[test]
    fn role_settings_resolve_both_explicit_source_forms() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("proof.bin"), [0, 255, 128]).unwrap();
        std::fs::write(dir.path().join("init.bin"), [10, 11, 12]).unwrap();
        let base64_yaml = dir.path().join("base64.yaml");
        std::fs::write(
            &base64_yaml,
            concat!(
                "role-1:\n  type: provisioned\n",
                "  membrane_proof:\n    base64: AQID\n",
                "  init_properties:\n    path: init.bin\n",
            ),
        )
        .unwrap();
        let path_yaml = dir.path().join("path.yaml");
        std::fs::write(
            &path_yaml,
            concat!(
                "role-1:\n  type: provisioned\n",
                "  membrane_proof:\n    path: proof.bin\n",
                "  init_properties:\n    base64: CgsM\n",
            ),
        )
        .unwrap();

        let resolved = read_role_settings_yaml(&base64_yaml).unwrap();
        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = resolved.get("role-1").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[1, 2, 3]);
        assert_eq!(init_properties.as_ref().unwrap().0.bytes(), &[10, 11, 12]);

        let resolved = read_role_settings_yaml(&path_yaml).unwrap();
        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = resolved.get("role-1").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[0, 255, 128]);
        assert_eq!(init_properties.as_ref().unwrap().0.bytes(), &[10, 11, 12]);
    }

    #[test]
    fn role_settings_resolve_optional_and_absolute_sources() {
        let dir = tempfile::tempdir().unwrap();
        let empty_path = dir.path().join("empty.bin");
        std::fs::write(&empty_path, []).unwrap();
        let yaml_path = dir.path().join("roles.yaml");
        std::fs::write(
            &yaml_path,
            format!(
                concat!(
                    "role-null:\n  type: provisioned\n  membrane_proof: null\n",
                    "  init_properties: null\n",
                    "role-omitted:\n  type: provisioned\n",
                    "role-empty:\n  type: provisioned\n",
                    "  membrane_proof:\n    base64: \"\"\n",
                    "  init_properties:\n    path: empty.bin\n",
                    "role-absolute:\n  type: provisioned\n",
                    "  membrane_proof:\n    path: {}\n",
                    "  modifiers:\n    network_seed: preserved-seed\n",
                ),
                empty_path.display()
            ),
        )
        .unwrap();

        let roles = read_role_settings_yaml(&yaml_path).unwrap();
        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = roles.get("role-null").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert!(membrane_proof.is_none());
        assert!(init_properties.is_none());

        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = roles.get("role-omitted").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert!(membrane_proof.is_none());
        assert!(init_properties.is_none());

        let RoleSettings::Provisioned {
            membrane_proof,
            init_properties,
            ..
        } = roles.get("role-empty").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[] as &[u8]);
        assert_eq!(init_properties.as_ref().unwrap().0.bytes(), &[] as &[u8]);

        let RoleSettings::Provisioned {
            membrane_proof,
            modifiers,
            ..
        } = roles.get("role-absolute").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[] as &[u8]);
        assert_eq!(
            modifiers
                .as_ref()
                .and_then(|modifiers| modifiers.network_seed.as_deref()),
            Some("preserved-seed")
        );
    }

    #[test]
    fn role_settings_resolution_errors_include_file_role_field_and_path() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_path = dir.path().join("roles.yaml");
        let missing_path = dir.path().join("missing-proof.bin");
        std::fs::write(
            &yaml_path,
            "role-1:\n  type: provisioned\n  membrane_proof:\n    path: missing-proof.bin\n",
        )
        .unwrap();

        let error = read_role_settings_yaml(&yaml_path).unwrap_err();
        let RoleSettingsYamlError::Settings { path, source } = &error else {
            panic!("expected settings context, got {error:?}");
        };
        assert_eq!(path, &yaml_path);
        let RoleSettingsYamlError::Role { role, source } = source.as_ref() else {
            panic!("expected role context, got {source:?}");
        };
        assert_eq!(role, "role-1");
        let RoleSettingsYamlError::Field { field, source } = source.as_ref() else {
            panic!("expected field context, got {source:?}");
        };
        assert_eq!(*field, "membrane_proof");
        let OpaqueBytesSourceError::Read { path, .. } = source else {
            panic!("expected file-read error, got {source:?}");
        };
        assert_eq!(path, &missing_path);
        let rendered = format!("{error:?}");
        assert!(rendered.contains("roles.yaml"));
        assert!(rendered.contains("role-1"));
        assert!(rendered.contains("membrane_proof"));
        assert!(rendered.contains("missing-proof.bin"));
    }

    #[test]
    fn role_settings_reports_invalid_base64_and_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let invalid_base64 = dir.path().join("invalid.yaml");
        std::fs::write(
            &invalid_base64,
            "role-1:\n  type: provisioned\n  init_properties:\n    base64: 'not valid !!!'\n",
        )
        .unwrap();
        let error = read_role_settings_yaml(&invalid_base64).unwrap_err();
        let RoleSettingsYamlError::Settings { source, .. } = error else {
            panic!("expected settings context");
        };
        let RoleSettingsYamlError::Role { source, .. } = *source else {
            panic!("expected role context");
        };
        let RoleSettingsYamlError::Field { field, source } = *source else {
            panic!("expected field context");
        };
        assert_eq!(field, "init_properties");
        assert!(matches!(source, OpaqueBytesSourceError::Base64(_)));

        let missing_settings = dir.path().join("missing.yaml");
        let error = read_role_settings_yaml(&missing_settings).unwrap_err();
        assert!(matches!(
            error,
            RoleSettingsYamlError::Read { path, .. } if path == missing_settings
        ));

        let malformed = dir.path().join("malformed.yaml");
        std::fs::write(&malformed, "role-1: [").unwrap();
        let error = read_role_settings_yaml(&malformed).unwrap_err();
        assert!(matches!(
            error,
            RoleSettingsYamlError::Parse { path, .. } if path == malformed
        ));
    }

    #[test]
    #[expect(deprecated, reason = "preserve existing-cell role settings")]
    fn role_settings_resolve_existing_cell_unchanged() {
        let cell_id = CellId::new(fixt!(DnaHash), fixt!(AgentPubKey));
        let resolved = RoleSettingsYaml::UseExisting {
            cell_id: cell_id.clone(),
        }
        .resolve(Path::new("."))
        .unwrap();
        assert!(
            matches!(resolved, RoleSettings::UseExisting { cell_id: actual } if actual == cell_id)
        );
    }

    #[test]
    fn role_settings_proof_overrides_without_input_return_none() {
        assert!(read_role_settings_yaml_with_proofs(None, &[])
            .unwrap()
            .is_none());
    }

    #[test]
    fn role_settings_proof_overrides_merge_from_invocation_directory() {
        let current_dir = std::env::current_dir().unwrap();
        let fixture_dir = tempfile::tempdir_in(&current_dir).unwrap();
        let settings_dir = fixture_dir.path().join("settings");
        std::fs::create_dir(&settings_dir).unwrap();
        let settings_path = settings_dir.join("roles.yaml");
        let yaml_relative_proof = settings_dir.join("proof.bin");
        std::fs::write(&yaml_relative_proof, [9, 9, 9]).unwrap();
        let flag_proof = fixture_dir.path().join("proof.bin");
        let expected = vec![0, 255, 128, 10];
        std::fs::write(&flag_proof, &expected).unwrap();
        std::fs::write(
            &settings_path,
            concat!(
                "role-1:\n  type: provisioned\n  membrane_proof: null\n",
                "  modifiers:\n    network_seed: preserved-seed\n",
                "  init_properties:\n    base64: AQID\n",
            ),
        )
        .unwrap();
        let flag_path = flag_proof.strip_prefix(&current_dir).unwrap();
        let roles = read_role_settings_yaml_with_proofs(
            Some(&settings_path),
            &[("role-1".into(), flag_path.to_path_buf())],
        )
        .unwrap()
        .unwrap();
        let RoleSettings::Provisioned {
            membrane_proof,
            modifiers,
            init_properties,
        } = roles.get("role-1").unwrap()
        else {
            panic!("expected a provisioned role");
        };
        assert_eq!(
            membrane_proof.as_ref().unwrap().bytes(),
            expected.as_slice()
        );
        assert_eq!(
            modifiers
                .as_ref()
                .and_then(|modifiers| modifiers.network_seed.as_deref()),
            Some("preserved-seed")
        );
        assert_eq!(init_properties.as_ref().unwrap().0.bytes(), &[1, 2, 3]);
    }

    #[test]
    fn role_settings_proof_overrides_support_multiple_flag_only_roles() {
        let first = tempfile::NamedTempFile::new().unwrap();
        let second = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(first.path(), [1, 2]).unwrap();
        std::fs::write(second.path(), [3, 4]).unwrap();
        let roles = read_role_settings_yaml_with_proofs(
            None,
            &[
                ("role-1".into(), first.path().to_path_buf()),
                ("role-2".into(), second.path().to_path_buf()),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles["role-1"].modifiers().is_none());
        let RoleSettings::Provisioned { membrane_proof, .. } = roles.get("role-1").unwrap() else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[1, 2]);
        let RoleSettings::Provisioned { membrane_proof, .. } = roles.get("role-2").unwrap() else {
            panic!("expected a provisioned role");
        };
        assert_eq!(membrane_proof.as_ref().unwrap().bytes(), &[3, 4]);
    }

    #[test]
    fn role_settings_proof_overrides_reject_conflicts_before_reads() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("roles.yaml");
        std::fs::write(
            &settings_path,
            "role-1:\n  type: provisioned\n  membrane_proof:\n    base64: AQID\n",
        )
        .unwrap();
        let error = read_role_settings_yaml_with_proofs(
            Some(&settings_path),
            &[("role-1".into(), dir.path().join("missing.bin"))],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RoleSettingsYamlError::ProofConflict { role } if role == "role-1"
        ));

        let dna_hash = DnaHash::from_raw_36(vec![0xdb; 36]);
        let agent_key = AgentPubKey::from_raw_36(vec![0xaa; 36]);
        let dna = dna_hash
            .get_raw_39()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let agent = agent_key
            .get_raw_39()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let existing_yaml =
            format!("role-1:\n  type: use_existing\n  cell_id: [[{dna}], [{agent}]]\n");
        std::fs::write(&settings_path, existing_yaml).unwrap();
        let error = read_role_settings_yaml_with_proofs(
            Some(&settings_path),
            &[("role-1".into(), dir.path().join("missing.bin"))],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RoleSettingsYamlError::ExistingCell { role } if role == "role-1"
        ));
    }

    #[test]
    fn role_settings_proof_overrides_reject_duplicate_roles() {
        let error = read_role_settings_yaml_with_proofs(
            None,
            &[
                ("role-1".into(), PathBuf::from("first.bin")),
                ("role-1".into(), PathBuf::from("second.bin")),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RoleSettingsYamlError::DuplicateProof { role } if role == "role-1"
        ));
    }

    #[test]
    fn opaque_bytes_source_rejects_ambiguous_or_unknown_shapes() {
        for yaml in [
            "{}",
            "base64: AQID\npath: proof.bin",
            "other: AQID",
            "base64: AQID\nother: ignored",
            "[256]",
            "[-1]",
            "base64: 123",
            "path: [proof.bin]",
        ] {
            assert!(yaml_serde::from_str::<OpaqueBytesSource>(yaml).is_err());
        }
    }

    #[test]
    fn opaque_bytes_source_round_trips_without_reading_files() {
        for input in [
            "base64: AQID",
            "path: /nonexistent/proof.bin",
            "[1, 2, 3]",
            "AQID",
        ] {
            let source: OpaqueBytesSource = yaml_serde::from_str(input).unwrap();
            let yaml = yaml_serde::to_string(&source).unwrap();
            let decoded: OpaqueBytesSource = yaml_serde::from_str(&yaml).unwrap();
            assert_eq!(source, decoded);
        }
    }
}
