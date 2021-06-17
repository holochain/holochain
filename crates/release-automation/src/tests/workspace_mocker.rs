use crate::*;

use anyhow::Context;
use cargo_test_support::git::{self, Repository};
use cargo_test_support::{Project, ProjectBuilder};
use log::debug;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile;

pub(crate) enum MockProjectType {
    Lib,
    Bin,
}

pub(crate) struct MockProject {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) dependencies: Vec<String>,
    pub(crate) excluded: bool,
    pub(crate) ty: MockProjectType,
    pub(crate) changelog: Option<String>,
}

pub(crate) struct WorkspaceMocker {
    pub(crate) dir: Option<tempfile::TempDir>,
    pub(crate) projects: HashMap<String, MockProject>,
    pub(crate) workspace_project: Project,
    pub(crate) workspace_repo: git2::Repository,
}

impl WorkspaceMocker {
    pub(crate) fn try_new(
        toplevel_changelog: Option<&str>,
        projects: Vec<MockProject>,
    ) -> Fallible<Self> {
        let (path, dir) = {
            let dir = tempfile::tempdir()?;
            if std::option_env!("KEEP_MOCK_WORKSPACE")
                .map(str::parse::<bool>)
                .map(Result::ok)
                .flatten()
                .unwrap_or_default()
            {
                debug!("keeping {:?}", dir.path());
                (dir.into_path(), None)
            } else {
                (dir.path().to_path_buf(), Some(dir))
            }
        };

        let projects = projects
            .into_iter()
            .map(|project| (project.name.clone(), project))
            .collect::<HashMap<_, _>>();

        let excluded = projects.iter().fold(String::new(), |acc, (name, project)| {
            if project.excluded {
                acc + &indoc::formatdoc!(
                    r#"
                        "crates/{}",
                    "#,
                    name
                )
            } else {
                acc
            }
        });

        let project_builder = ProjectBuilder::new(path).file(
            "Cargo.toml",
            &indoc::formatdoc!(
                r#"
                [workspace]
                members = [ "crates/*" ]
                exclude = [
                    {}
                ]
                "#,
                excluded
            ),
        );

        let project_builder = if let Some(toplevel_changelog) = toplevel_changelog {
            project_builder.file("CHANGELOG.md", toplevel_changelog)
        } else {
            project_builder
        };

        let project_builder =
            projects
                .iter()
                .fold(project_builder, |project_builder, (name, project)| {
                    use MockProjectType::{Bin, Lib};

                    let dependencies = project
                        .dependencies
                        .iter()
                        .fold(String::new(), |dependencies, dependency| {
                            format!("{}{}\n", dependencies, dependency)
                        });

                    let project_builder = project_builder
                        .file(
                            format!("crates/{}/Cargo.toml", &name),
                            &indoc::formatdoc!(
                                r#"
                                [project]
                                name = "{}"
                                version = "{}"
                                authors = []

                                [dependencies]
                                {}
                                "#,
                                &name,
                                &project.version,
                                dependencies
                            ),
                        )
                        .file(
                            format!(
                                "crates/{}/src/{}",
                                &name,
                                match &project.ty {
                                    Lib => "lib.rs",
                                    Bin => "main.rs",
                                }
                            ),
                            match &project.ty {
                                Lib => "",
                                Bin => "fn main() {}",
                            },
                        );

                    if let Some(changelog) = &project.changelog {
                        project_builder.file(format!("crates/{}/CHANGELOG.md", &name), &changelog)
                    } else {
                        project_builder
                    }
                });

        let workspace_project = project_builder.build();

        let workspace_mocker = Self {
            dir,
            projects,
            workspace_repo: git::init(&workspace_project.root()),
            workspace_project,
        };

        workspace_mocker.commit(None);

        Ok(workspace_mocker)
    }

    pub(crate) fn root(&self) -> std::path::PathBuf {
        self.workspace_project.root()
    }

    pub(crate) fn add_or_replace_file(&self, path: &str, content: &str) {
        self.workspace_project.change_file(path, content);
    }

    pub(crate) fn commit(&self, tag: Option<&str>) -> String {
        git::add(&self.workspace_repo);
        let commit = git::commit(&self.workspace_repo).to_string();

        if let Some(tag) = tag {
            let _ = self.tag(tag);
        }

        commit
    }

    pub(crate) fn tag(&self, tag: &str) -> () {
        git::tag(&self.workspace_repo, &tag)
    }

    pub(crate) fn head(&self) -> Fallible<String> {
        self.workspace_repo
            .revparse_single("HEAD")
            .context("revparse HEAD")
            .map(|o| o.id())
            .map(|id| id.to_string())
    }
}

/// A workspace with four crates to test changelogs and change detection.
pub(crate) fn example_workspace_1<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "crate_a".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string(),
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(
                indoc::indoc!(
                    r#"
                    ---
                    ---
                    # Changelog

                    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
                    This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

                    *Note: Versions 0.0.52-alpha2 and older are part belong to previous iterations of the Holochain architecture and are not tracked here.*

                    ## Unreleased

                    ### Added

                    - `InstallAppBundle` command added to admin conductor API. [#665](https://github.com/holochain/holochain/pull/665)
                    - `DnaSource` in conductor_api `RegisterDna` call now can take a `DnaBundle` [#665](https://github.com/holochain/holochain/pull/665)

                    ### Removed

                    - BREAKING:  `InstallAppDnaPayload` in admin conductor API `InstallApp` command now only accepts a hash.  Both properties and path have been removed as per deprecation warning.  Use either `RegisterDna` or `InstallAppBundle` instead. [#665](https://github.com/holochain/holochain/pull/665)
                    - BREAKING: `DnaSource(Path)` in conductor_api `RegisterDna` call now must point to `DnaBundle` as created by `hc dna pack` not a `DnaFile` created by `dna_util` [#665](https://github.com/holochain/holochain/pull/665)

                    ## 0.0.1

                    This is the first version number for the version of Holochain with a refactored state model (you may see references to it as Holochain RSM).
                    "#
                )
                .to_string(),
            ),
        },
        MockProject {
            name: "crate_b".to_string(),
            version: "0.0.1-alpha.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
                This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

                ## [Unreleased]

                ### Changed
                - `Signature` is a 64 byte 'secure primitive'

                ## 0.0.1-alpha.1

                [Unreleased]: https://github.com/holochain/holochain/holochain_zome_types-v0.0.2-alpha.1...HEAD
                "#
            )),
        },
        MockProject {
            name: "crate_c".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                r#"
                ---
                unreleasable: true
                default_unreleasable: true
                ---
                # Changelog
                Hello

                ## [Unreleased]
                Awesome changes!

                [Unreleased]: file:///dev/null
                "#
            )),
        },
        MockProject {
            name: "crate_d".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: true,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: None,
        },
        MockProject {
            name: "crate_e".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                Hello. This crate is releasable.

                ## [Unreleased]
                Awesome changes!

                [Unreleased]: file:///dev/null
                "#
            )),
        },
        MockProject {
            name: "crate_f".to_string(),
            version: "0.1.0".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                    r#"
                    # Changelog
                    Hello. This crate is releasable.

                    ## [Unreleased]
                    Awesome changes!

                    [Unreleased]: file:///dev/null
                    "#
                )),
        },
    ];

    let workspace_mocker = WorkspaceMocker::try_new(
        Some(indoc::indoc! {r#"
        # Changelog
        This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released.
        The file is updated every time one or more crates are released.

        The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
        This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

        # [Unreleased]
        The text beneath this heading will be retained which allows adding overarching release notes.

        ## Something outdated maybe
        This will be removed.

        ## [crate_a](crates/crate_a/CHANGELOG.md#unreleased)
        ### Added

        - `InstallAppBundle` command added to admin conductor API. [#665](https://github.com/holochain/holochain/pull/665)

        # [20210304.120604]
        This will include the hdk-0.0.100 release.

        ## [hdk-0.0.100](crates/hdk/CHANGELOG.md#0.0.100)

        ### Changed
        - hdk: fixup the autogenerated hdk documentation.
        "#
        }),
        members,
    )?;

    // todo: derive the tag from a function
    workspace_mocker.tag("crate_a-v0.0.1");
    workspace_mocker.add_or_replace_file(
        "crates/crate_a/README",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    // todo: derive the tag from a function
    workspace_mocker.tag("crate_b-v0.0.1-alpha.1");
    workspace_mocker.add_or_replace_file(
        "crates/crate_b/README",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    workspace_mocker.add_or_replace_file(
        "crates/crate_e/README",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    workspace_mocker.add_or_replace_file(
        "crates/crate_f/README",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    Ok(workspace_mocker)
}

/// A workspace to test dependencies and crate sorting.
pub(crate) fn example_workspace_2<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "crate_a".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string(),
                r#"crate_c = { path = "../crate_c", version = "0.0.1" }"#.to_string(),
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: None,
        },
        MockProject {
            name: "crate_b".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: None,
        },
        MockProject {
            name: "crate_c".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: None,
        },
        MockProject {
            name: "crate_d".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_a = { path = "../crate_a", version = "0.0.1" }"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: None,
        },
    ];

    WorkspaceMocker::try_new(None, members)
}

/// A workspace that is blocked by an unreleasable dependency.
pub(crate) fn example_workspace_3<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "crate_a".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                Hello. This crate is releasable.

                ## [Unreleased]
                Awesome changes!

                [Unreleased]: file:///dev/null
                "#
            )),
        },
        MockProject {
            name: "crate_b".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                // todo: debug dependency cycle
                // r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string(),
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                ---
                unreleasable: true
                ---
                # Changelog
                Hello. This crate is releasable.

                ## [Unreleased]
                Awesome changes!

                [Unreleased]: file:///dev/null
                "#
            )),
        },
    ];

    let workspace_mocker = WorkspaceMocker::try_new(None, members)?;

    Ok(workspace_mocker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example() {
        let workspace_mocker = example_workspace_1().unwrap();
        workspace_mocker.add_or_replace_file(
            "README",
            indoc::indoc! {r#"
            # Example

            Some changes
            "#,
            },
        );
        let before = workspace_mocker.head().unwrap();
        let after = workspace_mocker.commit(None);

        assert_ne!(before, after);
        assert_eq!(after, workspace_mocker.head().unwrap());
    }
}
