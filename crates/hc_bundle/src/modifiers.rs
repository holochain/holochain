//! Resolution of DNA modifier overrides for `hc dna hash`.

use holochain_types::prelude::{DnaModifiersOpt, SerializedBytes, YamlProperties};

/// Role settings for a single DNA, as read from the file passed to
/// `hc dna hash --role-settings`.
///
/// This matches the shape of one role's entry in the `roles-settings.yaml`
/// file accepted by `hc sandbox generate --roles-settings`, but scoped to a
/// single DNA: only the `modifiers` block is read, any other fields are
/// ignored so a full role entry can be pasted as-is.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct DnaRoleSettings {
    /// Modifier overrides to apply on top of the DNA manifest's own values.
    pub modifiers: Option<DnaModifiersOpt<YamlProperties>>,
}

/// Combine the `--network-seed` flag and the `--role-settings` file into the
/// modifier overrides to hash with.
///
/// Mirrors install-time precedence: the seed from the role settings file
/// overrides the `--network-seed` flag, just as role settings passed to
/// `InstallApp` override its `network_seed` field.
pub(crate) fn resolve_modifier_overrides(
    network_seed: Option<String>,
    role_settings: Option<DnaRoleSettings>,
) -> anyhow::Result<DnaModifiersOpt<SerializedBytes>> {
    let mut modifiers = DnaModifiersOpt::<YamlProperties>::none();
    modifiers.network_seed = network_seed;
    if let Some(settings) = role_settings {
        let Some(overrides) = settings.modifiers else {
            anyhow::bail!(
                "the role settings file does not contain a `modifiers` block, \
                 so there is nothing to override"
            );
        };
        if let Some(seed) = overrides.network_seed {
            modifiers.network_seed = Some(seed);
        }
        if let Some(properties) = overrides.properties {
            modifiers.properties = Some(properties);
        }
    }
    modifiers.serialized().map_err(anyhow::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::prelude::YamlProperties;

    fn yaml_settings(s: &str) -> DnaRoleSettings {
        yaml_serde::from_str(s).unwrap()
    }

    #[test]
    fn no_overrides_yields_none() {
        let m = resolve_modifier_overrides(None, None).unwrap();
        assert!(!m.has_some_option_set());
    }

    #[test]
    fn network_seed_flag_sets_seed() {
        let m = resolve_modifier_overrides(Some("seed-a".into()), None).unwrap();
        assert_eq!(m.network_seed.as_deref(), Some("seed-a"));
        assert!(m.properties.is_none());
    }

    #[test]
    fn role_settings_seed_wins_over_flag() {
        let settings = yaml_settings("modifiers:\n  network_seed: seed-b\n");
        let m = resolve_modifier_overrides(Some("seed-a".into()), Some(settings)).unwrap();
        assert_eq!(m.network_seed.as_deref(), Some("seed-b"));
    }

    #[test]
    fn role_settings_properties_match_install_encoding() {
        let settings = yaml_settings("modifiers:\n  properties:\n    foo: bar\n");
        let m = resolve_modifier_overrides(None, Some(settings)).unwrap();
        let expected: holochain_types::prelude::SerializedBytes =
            YamlProperties::new(yaml_serde::from_str("foo: bar").unwrap())
                .try_into()
                .unwrap();
        assert_eq!(m.properties, Some(expected));
    }

    #[test]
    fn full_role_block_with_unknown_fields_parses() {
        let settings = yaml_settings(
            "type: provisioned\nmembrane_proof: ~\nmodifiers:\n  network_seed: seed-c\n",
        );
        let m = resolve_modifier_overrides(None, Some(settings)).unwrap();
        assert_eq!(m.network_seed.as_deref(), Some("seed-c"));
    }

    #[test]
    fn settings_without_modifiers_block_errors() {
        let settings = yaml_settings("membrane_proof: ~\n");
        assert!(resolve_modifier_overrides(None, Some(settings)).is_err());
    }
}
