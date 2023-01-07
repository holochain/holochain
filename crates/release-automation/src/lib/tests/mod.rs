pub mod cli;
pub mod workspace_mocker;

use std::path::Path;

use chrono::TimeZone;
use linked_hash_set::LinkedHashSet;

use crate::release::{ReleaseSteps, ReleaseWorkspace};

#[test]
fn release_steps_are_ordered() {
    let input = r"
        CreateReleaseBranch,
        BumpReleaseVersions,
        PublishToCratesIo,
        AddOwnersToCratesIo,
    ";

    let parsed = super::cli::parse_releasesteps(input)
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();

    assert_eq!(parsed.get(0), Some(&ReleaseSteps::CreateReleaseBranch));

    assert_eq!(parsed.get(1), Some(&ReleaseSteps::BumpReleaseVersions));

    assert_eq!(parsed.last(), Some(&ReleaseSteps::AddOwnersToCratesIo));
}

#[test]
fn parse_publish_error() {
    use crate::release::PublishError;

    struct TestCase<'a> {
        package: &'a str,
        version: &'a str,
        string: &'a str,
        expected_error: PublishError,
    }

    let cases = vec![
        TestCase {
            package: "",
            version: "",
            string: indoc::indoc!(
                r#"
                Caused by:
                cargo publish failed for the following paths:
                "#,
            ),

            expected_error: PublishError::Other(
                "".to_string(),
                indoc::formatdoc!(
                    r#"
                Caused by:
                cargo publish failed for the following paths:
                "#
                ),
            ),
        },
        TestCase {
            package: "kitsune_p2p_proxy",
            version: "0.0.1",
            string: indoc::indoc!(
                r#"
        "/home/steveej/src/holo/holochain_release/crates/kitsune_p2p/proxy/Cargo.toml":
           error: failed to prepare local package for uploading

           Caused by:
             no matching package named `kitsune_p2p_transport_quic` found
             location searched: registry `https://github.com/rust-lang/crates.io-index`
             required by package `kitsune_p2p_proxy v0.0.1 (/home/steveej/src/holo/holochain_release/crates/kitsune_p2p/proxy)`
                "#
            ),

            expected_error: PublishError::PackageNotFound {
                package: "kitsune_p2p_proxy".to_string(),
                version: "0.0.1".to_string(),
                path:
                    "/home/steveej/src/holo/holochain_release/crates/kitsune_p2p/proxy/Cargo.toml"
                        .to_string(),

                dependency: "kitsune_p2p_transport_quic".to_string(),
                location: "registry `https://github.com/rust-lang/crates.io-index`".to_string(),
                package_found: "kitsune_p2p_proxy".to_string(),
            },
        },
        TestCase {
            package: "holochain",
            version: "0.0.100",
            string: indoc::indoc!(
                r#"
                "/home/steveej/src/holo/holochain_release/crates/holochain/Cargo.toml":
                error: failed to prepare local package for uploading

                Caused by:
                    failed to select a version for the requirement `hdk = "^0.0.101-alpha.0"`
                    candidate versions found which didn't match: 0.0.100
                    location searched: crates.io index
                    required by package `holochain v0.0.100 (/home/steveej/src/holo/holochain_release/crates/holochain)`
                "#
            ),

            expected_error: PublishError::PackageVersionNotFound {
                package: "holochain".to_string(),
                version: "0.0.100".to_string(),
                path: "/home/steveej/src/holo/holochain_release/crates/holochain/Cargo.toml"
                    .to_string(),
                dependency: "hdk".to_string(),
                version_req: "^0.0.101-alpha.0".to_string(),
                location: "crates.io index".to_string(),
                package_found: "holochain".to_string(),
                version_found: "0.0.100".to_string(),
            },
        },
        TestCase {
            package: "crate_a",
            version: "0.0.2",
            string: indoc::indoc!(
                r#"
                error: failed to prepare local package for uploading

                Caused by:
                    no matching package named `crate_b` found
                    location searched: registry `https://github.com/rust-lang/crates.io-index`
                    required by package `crate_a v0.0.2 (/tmp/tmp.Oqk7lmGgfW/.tmp5m2olB/crates/crate_a)`
                "#
            ),

            expected_error: PublishError::PackageNotFound {
                package: "crate_a".to_string(),
                version: "0.0.2".to_string(),
                path: "".to_string(),

                dependency: "crate_b".to_string(),
                location: "registry `https://github.com/rust-lang/crates.io-index`".to_string(),
                package_found: "crate_a".to_string(),
            },
        },
        TestCase {
            package: "",
            version: "0.0.3",
            string: indoc::indoc!(
                r#"
                error: failed to publish to registry at https://crates.io

                Caused by:
                    the remote server responded with an error: crate version `0.0.3` is already uploaded
                "#
            ),

            expected_error: PublishError::AlreadyUploaded {
                package: "".to_string(),
                version: "0.0.3".to_string(),
                path: "".to_string(),
                location: "registry at https://crates.io".to_string(),
                version_found: "0.0.3".to_string(),
            },
        },
        TestCase {
            package: "",
            version: "",
            string: indoc::indoc!(
                r#"
                error: failed to publish to registry at https://crates.io

                       Caused by:
                         the remote server responded with an error (status 429 Too Many Requests): You have published too many crates in a short period of time. Please try again after Wed, 30 Jun 2021 21:09:24 GMT or email help@crates.io to have your limit increased.
                "#
            ),

            expected_error: PublishError::PublishLimitExceeded {
                package: "".to_string(),
                version: "".to_string(),
                location: "registry at https://crates.io".to_string(),
                retry_after: DateTime::<Utc>::from_utc(NaiveDateTime::parse_from_str("2021-06-30 21:09:24", "%Y-%m-%d %H:%M:%S", Utc),
            },
        },
    ];

    for case in cases {
        let result = PublishError::with_str(
            case.package.to_string(),
            case.version.to_string(),
            case.string.to_string(),
        );

        assert_eq!(case.expected_error, result);
    }
}
