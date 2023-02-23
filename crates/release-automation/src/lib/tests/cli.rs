use std::env::temp_dir;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::changelog::{sanitize, Frontmatter};
use crate::changelog::{ChangelogT, CrateChangelog, WorkspaceChangelog};
use crate::common::SemverIncrementMode;
use crate::crate_selection::ReleaseWorkspace;
use crate::tests::workspace_mocker::{
    example_workspace_1, example_workspace_1_aggregated_changelog, example_workspace_4,
};
use crate::Fallible;
use anyhow::Context;
use once_cell::sync::OnceCell;
use predicates::prelude::*;
use serde::Deserialize;
use std::io::Write;

/// uses a shared temporary directory for all Commands that and sets their HOME and CARGO_HOME respectively.
/// optionally changes the working directory into the given path.
pub(crate) fn command_pure(
    program: &str,
    maybe_cwd: Option<&Path>,
) -> Fallible<assert_cmd::Command> {
    static TMP_HOME: once_cell::sync::Lazy<tempfile::TempDir> =
        once_cell::sync::Lazy::new(|| tempfile::tempdir().unwrap());

    let home = TMP_HOME.path().join("home");

    std::fs::create_dir_all(&home)?;

    let mut cmd = assert_cmd::Command::new(program);
    cmd.env("HOME", home.as_path())
        .env("CARGO_HOME", home.join(".cargo"));

    if let Some(cwd) = maybe_cwd {
        cmd.current_dir(cwd);
    }

    Ok(cmd)
}

#[test]
fn release_createreleasebranch() {
    let workspace_mocker = example_workspace_1().unwrap();
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();
    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "release",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--steps=CreateReleaseBranch",
    ]);
    cmd.assert().success();

    crate::release::ensure_release_branch(&workspace).unwrap();
}

#[test]
fn release_createreleasebranch_fails_on_dirty_repo() {
    let workspace_mocker = example_workspace_1().unwrap();
    workspace_mocker.add_or_replace_file(
        "crates/crate_a/README",
        indoc::indoc! {r#"
            # Example

            Some changes
            More changes
            "#,
        },
    );
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();

    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=debug",
        "release",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--steps=CreateReleaseBranch",
    ]);

    cmd.assert()
        .stderr(predicate::str::contains("repository is not clean"))
        .failure();
}

#[macro_export]
macro_rules! assert_cmd_success {
    ($cmd:expr) => {{
        let output = $cmd.output().unwrap();

        let (stderr, stdout) = (
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout),
        );

        if !output.status.success() {
            panic!(
                "code: {:?}\nstderr:\n'{}'\n---\nstdout:\n'{}'\n---\n",
                output.status.code(),
                stderr,
                stdout,
            );
        };

        (String::from(stderr), String::from(stdout))
    }};
}

fn get_crate_versions<'a>(
    expected_crates: &[&str],
    workspace: &'a ReleaseWorkspace<'a>,
) -> Vec<String> {
    expected_crates
        .clone()
        .iter()
        .map(|name| {
            let cargo_toml_path = workspace
                .root()
                .join("crates")
                .join(name)
                .join("Cargo.toml");
            cargo_next::get_version(&cargo_toml_path)
                .context(format!(
                    "trying to parse version in Cargo.toml at {:?}",
                    cargo_toml_path
                ))
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>()
}

// todo(backlog): ensure all of these conditions have unit tests?
#[test]
fn bump_versions_on_selection() {
    let workspace_mocker = example_workspace_1().unwrap();
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();

    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "release",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--disallowed-version-reqs=>=0.2",
        "--allowed-matched-blockers=UnreleasableViaChangelogFrontmatter,DisallowedVersionReqViolated",
        "--steps=CreateReleaseBranch,BumpReleaseVersions",
        "--allowed-missing-dependencies=crate_b",
    ]);

    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

    // set expectations
    let expected_crates = vec!["crate_b", "crate_a", "crate_e"];
    let expected_release_versions = vec!["0.0.0", "0.1.0", "0.0.1"];

    // check manifests for new release headings
    assert_eq!(
        expected_release_versions,
        get_crate_versions(&expected_crates, &workspace),
    );

    // ensure *all* dependants were updated
    // alas, after refactoring the code into a loop i noticed there's only one dependency in this example workspace
    for (name, dep_name, expected_crate_version) in &[("crate_a", "crate_b", "=0.0.0")] {
        assert_eq!(
            expected_crate_version,
            &crate::common::get_dependency_version(
                &workspace
                    .root()
                    .join("crates")
                    .join(name)
                    .join("Cargo.toml"),
                dep_name
            )
            .unwrap()
            .replace("\"", "")
            .replace("\\", "")
            .replace(" ", ""),
        );
    }

    // check changelogs for new release headings
    assert_eq!(
        expected_release_versions,
        expected_crates
            .iter()
            .map(|crt| {
                let path = workspace
                    .root()
                    .join("crates")
                    .join(crt)
                    .join("CHANGELOG.md");
                ChangelogT::<CrateChangelog>::at_path(&path)
                    .topmost_release()
                    .context(format!("querying topmost_release on changelog for {}", crt))
                    .unwrap()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "changelog for {} doesn't have any release. file content: \n{}",
                            crt,
                            std::fs::read_to_string(&path).unwrap()
                        )
                    })
                    .map(|change| change.title().to_string())
                    .unwrap()
            })
            .collect::<Vec<String>>()
    );

    let workspace_changelog =
        ChangelogT::<WorkspaceChangelog>::at_path(&workspace.root().join("CHANGELOG.md"));
    let topmost_workspace_release = workspace_changelog
        .topmost_release()
        .unwrap()
        .map(|change| change.title().to_string())
        .unwrap();

    // todo: ensure the returned title doesn't contain any brackets?
    assert!(
        // FIXME: make this compatible with years beyond 2099
        topmost_workspace_release.starts_with("[20") || topmost_workspace_release.starts_with("20"),
        "{}",
        topmost_workspace_release
    );
    assert_ne!("[20210304.120604]", topmost_workspace_release);

    {
        // check for release heading contents in the workspace changelog

        let expected = sanitize(indoc::formatdoc!(
            r#"
        # Changelog

        This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released. The file is updated every time one or more crates are released.

        The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

        # \[Unreleased\]

        ## Something outdated maybe

        This will be removed by aggregation.

        ## [crate_c](crates/crate_c/CHANGELOG.md#unreleased)
        Awesome changes!

        ### Breaking
        Breaking changes, be careful.

        ## [crate_f](crates/crate_f/CHANGELOG.md#unreleased)
        This will be released in the future.

        # {}

        The text beneath this heading will be retained which allows adding overarching release notes.

        ## [crate_e-0.0.1](crates/crate_e/CHANGELOG.md#0.0.1)

        Awesome changes\!

        ## [crate_a-0.1.0](crates/crate_a/CHANGELOG.md#0.1.0)

        ### Added

        - `InstallAppBundle`
        - `DnaSource`

        ### Removed

        - BREAKING:  `InstallAppDnaPayload`
        - BREAKING: `DnaSource(Path)`

        ## [crate_b-0.0.0](crates/crate_b/CHANGELOG.md#0.0.0)

        ### Changed

        - `Signature` is a 64 byte ‘secure primitive’

        # \[20210304.120604\]

        This will include the hdk-0.0.100 release.

        ## [hdk-0.0.100](crates/hdk/CHANGELOG.md#0.0.100)

        ### Changed

        - hdk: fixup the autogenerated hdk documentation.
        "#,
            topmost_workspace_release
        ));

        let result = sanitize(std::fs::read_to_string(workspace_changelog.path()).unwrap());
        assert_eq!(
            result,
            expected,
            "{}",
            prettydiff::text::diff_lines(&result, &expected).format()
        );
    }

    // ensure the git commit for the whole release was created
    let commit_msg = {
        let commit = workspace
            .git_repo()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap();

        commit.message().unwrap().to_string()
    };

    assert_eq!(
        indoc::formatdoc!(
            r#"
        create a release from branch release-{}

        the following crates are part of this release:

        - crate_b-0.0.0
        - crate_a-0.1.0
        - crate_e-0.0.1
        "#,
            topmost_workspace_release
        ),
        commit_msg
    );

    // TODO: tag creation has been moved to publishing, test it there
    // ensure the git tags for the crate releases were created
    // for expected_tag in &["crate_b-0.0.0", "crate_a-0.0.2", "crate_e-0.0.1"] {
    //     crate::crate_selection::git_lookup_tag(workspace.git_repo(), &expected_tag)
    //         .expect(&format!("git tag '{}' not found", &expected_tag));
    // }

    if matches!(option_env!("FAIL_CLI_RELEASE_TEST"), Some(_)) {
        println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

        panic!("workspace root: {:?}", workspace.root());
    }
}

#[test]
fn changelog_aggregation() {
    let workspace_mocker = example_workspace_1().unwrap();

    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();

    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "changelog",
        "aggregate",
    ]);

    let _output = assert_cmd_success!(cmd);

    let workspace_changelog =
        ChangelogT::<WorkspaceChangelog>::at_path(&workspace.root().join("CHANGELOG.md"));
    let result = sanitize(std::fs::read_to_string(workspace_changelog.path()).unwrap());

    let expected = example_workspace_1_aggregated_changelog();
    assert_eq!(
        result,
        expected,
        "{}",
        prettydiff::text::diff_lines(&result, &expected).format()
    );
}

#[test]
fn release_publish() {
    let workspace_mocker = example_workspace_1().unwrap();
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();

    // simulate a release
    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "release",
        &format!("--cargo-target-dir={}", workspace.root().join("target").display()),
        "--disallowed-version-reqs=>=0.1",
        "--allowed-matched-blockers=UnreleasableViaChangelogFrontmatter,DisallowedVersionReqViolated",
        "--steps=CreateReleaseBranch,BumpReleaseVersions",
    ]);
    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

    // publish
    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "release",
        // todo: set up a custom registry and actually publish the crates
        "--dry-run",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--steps=PublishToCratesIo",
    ]);
    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);
}

// the post release version bump functionliaty has been removed from the release
// it now lives in a separate command and these tests can be moved there
#[ignore]
#[test]
fn post_release_version_bumps() {
    let workspace_mocker = example_workspace_1().unwrap();
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();

    // simulate a release
    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "release",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--disallowed-version-reqs=>=0.1",
        "--allowed-matched-blockers=UnreleasableViaChangelogFrontmatter,DisallowedVersionReqViolated",
        "--steps=CreateReleaseBranch,BumpReleaseVersions",
        "--allowed-missing-dependencies=crate_b",
    ]);
    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace.root().display()),
        "--log-level=trace",
        "release",
        &format!(
            "--cargo-target-dir={}",
            workspace.root().join("target").display()
        ),
        "--steps=BumpPostReleaseVersions",
    ]);
    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

    assert_eq!(
        vec!["0.0.2-dev.0", "0.0.3-dev.0", "0.0.2-dev.0"],
        get_crate_versions(&["crate_b", "crate_a", "crate_e"], &workspace),
    );

    // ensure the git commit for the whole release was created
    let commit_msg = {
        let commit = workspace
            .git_repo()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap();

        commit.message().unwrap().to_string()
    };

    let topmost_release_title = match workspace
        .changelog()
        .unwrap()
        .topmost_release()
        .unwrap()
        .unwrap()
    {
        crate::changelog::ReleaseChange::WorkspaceReleaseChange(title, _) => title,
        _ => "not found".to_string(),
    };

    assert!(
        commit_msg.starts_with(&format!(
            "setting develop versions to conclude 'release-{}'",
            topmost_release_title
        )),
        "unexpected commit msg: \n{}\n{}",
        commit_msg,
        topmost_release_title,
    );

    {
        // ensure the workspace release tag has been created
        let topmost_workspace_release = workspace
            .changelog()
            .unwrap()
            .topmost_release()
            .unwrap()
            .map(|change| change.title().to_string())
            .unwrap();
        let expected_tag = format!("release-{}", topmost_workspace_release);
        crate::crate_selection::git_lookup_tag(workspace.git_repo(), &expected_tag)
            .unwrap_or_else(|| panic!("git tag '{}' not found", &expected_tag));
    }
}

#[test]
fn multiple_subsequent_releases() {
    let workspace_mocker = example_workspace_1().unwrap();
    type A = (PathBuf, Vec<String>, Vec<String>);
    type F = Box<dyn Fn(A)>;

    for (
        i,
        (
            description,
            expected_versions,
            expected_crates,
            allowed_missing_dependencies,
            expect_new_release,
            pre_release_fn,
        ),
    ) in [
        (
            "bump the first time as they're initially released",
            vec!["0.0.0", "0.1.0", "0.0.1"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            Vec::<&str>::new(),
            true,
            Box::new(|_| {}) as F,
        ),
        (
            "should not bump the second time without making any changes",
            vec!["0.0.0", "0.1.0", "0.0.1"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            Vec::<&str>::new(),
            false,
            Box::new(|_| {}) as F,
        ),
        (
            "only crate_a and crate_e have changed, expect these to be bumped",
            vec!["0.0.0", "0.1.1", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // crate_b won't be part of the release so we allow it to be missing as we're not publishing
            vec!["crate_b"],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_a", "crate_e"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();

                    writeln!(readme, "A new line!").unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
        (
            "change crate_b, and as crate_a depends on crate_b it'll be bumped as well",
            vec!["0.0.1", "0.1.2", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            vec![],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_b"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();
                    writeln!(readme, "A new line!").unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
        (
            "add a pre-release for crate_b",
            vec!["1.0.0-rc.0", "0.1.3", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            vec![],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_b"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();
                    writeln!(readme, "A new line!").unwrap();

                    ChangelogT::<CrateChangelog>::at_path(
                        &root.join(format!("crates/{}/CHANGELOG.md", crt)),
                    )
                    .set_front_matter(
                        &serde_yaml::from_str(
                            indoc::formatdoc!(
                                r#"
                                default_semver_increment_mode: !pre_major rc
                                "#
                            )
                            .as_str(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
        (
            "do another pre-release for crate_b",
            vec!["1.0.0-rc.1", "0.1.4", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            vec![],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_b"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();
                    writeln!(readme, "A new line!").unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
        (
            "do major release for crate_b",
            vec!["1.0.0", "0.1.5", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            vec![],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_b"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();
                    writeln!(readme, "A new line!").unwrap();

                    ChangelogT::<CrateChangelog>::at_path(
                        &root.join(format!("crates/{}/CHANGELOG.md", crt)),
                    )
                    .set_front_matter(
                        &serde_yaml::from_str(
                            indoc::formatdoc!(
                                r#"
                                semver_increment_mode: major
                                "#
                            )
                            .as_str(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
        (
            "and a default patch release for crate_b again",
            vec!["1.0.1", "0.1.6", "0.0.2"],
            vec!["crate_b", "crate_a", "crate_e"],
            // allowed missing dependencies
            vec![],
            true,
            Box::new(|args: A| {
                let root = args.0;

                for crt in &["crate_b"] {
                    let mut readme = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(root.join(format!("crates/{}/README.md", crt)))
                        .unwrap();
                    writeln!(readme, "A new line!").unwrap();
                }

                ReleaseWorkspace::try_new(root)
                    .unwrap()
                    .git_add_all_and_commit("some chnages", None)
                    .unwrap();
            }) as F,
        ),
    ]
    .iter()
    .enumerate()
    {
        println!("---\ntest case {}\n---", i);

        pre_release_fn((
            workspace_mocker.root(),
            expected_versions
                .clone()
                .into_iter()
                .map(String::from)
                .collect(),
            expected_crates
                .clone()
                .into_iter()
                .map(String::from)
                .collect(),
        ));

        let topmost_release_title_pre = {
            let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
            let topmost_release_title_pre = match workspace
                .changelog()
                .unwrap()
                .topmost_release()
                .unwrap()
                .unwrap()
            {
                crate::changelog::ReleaseChange::WorkspaceReleaseChange(title, _) => title,
                _ => "not found".to_string(),
            };

            workspace.git_checkout_branch("develop", true).unwrap();

            let mut cmd =
                command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
            let cmd = cmd.args(&[
                &format!("--workspace-path={}", workspace.root().display()),
                "--log-level=trace",
                "--match-filter=crate_(a|b|e)",
                "release",
                &format!(
                    "--cargo-target-dir={}",
                    workspace.root().join("target").display()
                ),
                "--allowed-matched-blockers=UnreleasableViaChangelogFrontmatter",
                "--steps=CreateReleaseBranch,BumpReleaseVersions",
                &format!(
                    "--allowed-missing-dependencies={}",
                    allowed_missing_dependencies
                        .iter()
                        .fold("".to_string(), |acc, cur| { acc + "," + *cur })
                ),
            ]);
            let output = assert_cmd_success!(cmd);
            println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1,);

            topmost_release_title_pre
        };

        let topmost_release_title = {
            // todo: figure out how we can make the workspace re-read its data instead of creating a new one
            let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();

            assert_eq!(
                expected_versions,
                &get_crate_versions(expected_crates, &workspace),
                "{} ({})",
                description,
                i
            );

            let topmost_release_title = match workspace
                .changelog()
                .unwrap()
                .topmost_release()
                .unwrap()
                .unwrap()
            {
                crate::changelog::ReleaseChange::WorkspaceReleaseChange(title, _) => title,
                _ => "not found".to_string(),
            };

            {
                // ensure the git commit matches the most recent release.

                let commit_msg = {
                    let commit = workspace
                        .git_repo()
                        .head()
                        .unwrap()
                        .peel_to_commit()
                        .unwrap();

                    commit.message().unwrap().to_string()
                };

                let expected_start = format!(
                    "create a release from branch release-{}",
                    topmost_release_title
                );

                assert!(
                    commit_msg.starts_with(&expected_start),
                    "unexpected commit msg. got: \n'{}'\nexpected it to start with: \n'{}'",
                    commit_msg,
                    expected_start,
                );
            }

            topmost_release_title
        };

        if *expect_new_release {
            assert_ne!(
                topmost_release_title, topmost_release_title_pre,
                "expected new release? {}",
                *expect_new_release
            );
        } else {
            assert_eq!(
                topmost_release_title, topmost_release_title_pre,
                "expected new release? {}",
                *expect_new_release
            )
        }

        // sleep so the time based branch name is unique
        // todo: change to other branch name generator?
        std::thread::sleep(std::time::Duration::new(1, 0));
    }

    if matches!(option_env!("FAIL_CLI_RELEASE_TEST"), Some(_)) {
        panic!("workspace root: {:?}", workspace_mocker.root());
    }
}

#[test]
fn apply_dev_versions_works() {
    let workspace_mocker = example_workspace_1().unwrap();

    let get_crate_a_version = || -> String {
        let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
        let crate_a = workspace
            .members()
            .unwrap()
            .iter()
            .find(|m| m.name() == "crate_a")
            .unwrap();

        crate_a.version().to_string()
    };

    assert_eq!(get_crate_a_version(), "0.0.1");

    let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
    let cmd = cmd.args(&[
        &format!("--workspace-path={}", workspace_mocker.root().display()),
        "--log-level=debug",
        "crate",
        "apply-dev-versions",
    ]);
    let output = assert_cmd_success!(cmd);
    println!("stderr:\n'{}'\n---\nstdout:\n'{}'\n---", output.0, output.1);

    assert_eq!(get_crate_a_version(), "0.0.2-dev.0");
}

#[test]
fn release_dry_run_fails_on_unallowed_conditions() {
    let workspace_mocker = example_workspace_4().unwrap();
    let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();
    workspace.git_checkout_new_branch("develop").unwrap();

    let members = workspace
        .members()
        .unwrap()
        .iter()
        .map(|m| m.name())
        .collect::<Vec<_>>();

    // every member corresponds to a blocker and must thus fail separately
    for member in members {
        workspace_mocker.add_or_replace_file(
            &format!("crates/{}/README.md", member),
            indoc::indoc! {r#"
            # Example

            Some changes
            More changes
            "#,
            },
        );

        workspace
            .update_lockfile(false, std::iter::empty())
            .unwrap();

        workspace.git_add_all_and_commit("msg", None).unwrap();

        let mut cmd = command_pure("release-automation", Some(&workspace_mocker.root())).unwrap();
        let cmd = cmd.args(&[
            &format!("--workspace-path={}", workspace.root().display()),
            &format!("--match-filter={}", member),
            "--log-level=debug",
            "release",
            "--dry-run",
            "--no-verify",
            "--steps=BumpReleaseVersions",
        ]);

        cmd.assert().failure();
    }
}
