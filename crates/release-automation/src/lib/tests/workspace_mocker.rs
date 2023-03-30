use crate::*;

use anyhow::{bail, Context};
use cargo_test_support::git::{self, Repository};
use cargo_test_support::paths::init_root;
use cargo_test_support::{Project, ProjectBuilder};
use educe::Educe;
use log::debug;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Educe)]
#[educe(Default)]
pub enum MockProjectType {
    #[educe(Default)]
    Lib,
    Bin,
}

#[derive(Educe)]
#[educe(Default)]
pub struct MockProject {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub dev_dependencies: Vec<String>,
    pub excluded: bool,
    pub ty: MockProjectType,
    pub changelog: Option<String>,
    #[educe(Default(expression = r##"Some(indoc::formatdoc!(
                r#"
                # README
                "#
            ))
    "##))]
    pub readme: Option<String>,
    #[educe(Default(expression = r##"Some("some crate".to_string())"##))]
    pub description: Option<String>,
    #[educe(Default(expression = r##"Some("Apache-2.0".to_string())"##))]
    pub license: Option<String>,
    pub keywords: Vec<String>,
}

pub struct WorkspaceMocker {
    pub dir: Option<tempfile::TempDir>,
    pub projects: HashMap<String, MockProject>,
    pub workspace_project: Project,
    pub workspace_repo: git2::Repository,
}

impl WorkspaceMocker {
    pub fn try_new(toplevel_changelog: Option<&str>, projects: Vec<MockProject>) -> Fallible<Self> {
        init_root(None);

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
                acc + indoc::formatdoc!(
                    r#"
                        "crates/{}",
                    "#,
                    name
                )
                .as_str()
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

                    let dev_dependencies = project.dev_dependencies.iter().fold(
                        String::new(),
                        |dev_dependencies, dependency| {
                            format!("{}{}\n", dev_dependencies, dependency)
                        },
                    );

                    let keywords = project
                        .keywords
                        .iter()
                        .map(|keyword| format!(r#""{}""#, keyword))
                        .collect::<Vec<_>>()
                        .join(",");

                    let project_builder = project_builder
                        .file(
                            format!("crates/{}/Cargo.toml", &name),
                            &indoc::formatdoc!(
                                r#"
                                [package]
                                name = "{}"
                                version = "{}"
                                authors = []
                                {description}
                                {license}
                                homepage = "https://github.com/holochain/holochain"
                                documentation = "https://github.com/holochain/holochain"
                                keywords = [{keywords}]

                                [dependencies]
                                {dependencies}

                                [dev-dependencies]
                                {dev_dependencies}
                                "#,
                                &name,
                                &project.version,
                                description = &project
                                    .description
                                    .clone()
                                    .map(|d| format!(r#"description = "{}""#, d))
                                    .unwrap_or_default(),
                                license = &project
                                    .license
                                    .clone()
                                    .map(|d| format!(r#"license = "{}""#, d))
                                    .unwrap_or_default(),
                                dependencies = dependencies,
                                dev_dependencies = dev_dependencies,
                                keywords = keywords,
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

                    let project_builder = if let Some(changelog) = &project.changelog {
                        project_builder.file(format!("crates/{}/CHANGELOG.md", &name), changelog)
                    } else {
                        project_builder
                    };

                    if let Some(readme) = &project.readme {
                        project_builder.file(format!("crates/{}/README.md", &name), readme)
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

    pub fn root(&self) -> std::path::PathBuf {
        self.workspace_project.root()
    }

    pub fn add_or_replace_file(&self, path: &str, content: &str) {
        self.workspace_project.change_file(path, content);
    }

    pub fn commit(&self, tag: Option<&str>) -> String {
        git::add(&self.workspace_repo);
        let commit = git::commit(&self.workspace_repo).to_string();

        if let Some(tag) = tag {
            let _ = self.tag(tag);
        }

        commit
    }

    pub fn tag(&self, tag: &str) {
        git::tag(&self.workspace_repo, tag)
    }

    pub fn head(&self) -> Fallible<String> {
        self.workspace_repo
            .revparse_single("HEAD")
            .context("revparse HEAD")
            .map(|o| o.id())
            .map(|id| id.to_string())
    }

    pub fn update_lockfile(&self) -> Fallible<()> {
        let mut cmd = std::process::Command::new("cargo");
        cmd.args(
            &[vec![
                "update",
                "--workspace",
                "--offline",
                "--verbose",
                "--manifest-path",
                &format!("{}/Cargo.toml", self.root().to_string_lossy()),
            ]]
            .concat(),
        );
        debug!("running command: {:?}", cmd);

        let mut cmd = cmd.spawn()?;
        let cmd_status = cmd.wait()?;
        if !cmd_status.success() {
            bail!("running {:?} failed: \n{:?}", cmd, cmd.stderr);
        }

        Ok(())
    }
}

/// Expected changelog after aggregation.
pub fn example_workspace_1_aggregated_changelog() -> String {
    crate::changelog::sanitize(indoc::formatdoc!(
        r#"
        # Changelog
        This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released.
        The file is updated every time one or more crates are released.

        The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
        This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

        # [Unreleased]
        The text beneath this heading will be retained which allows adding overarching release notes.

        ## [crate_b](crates/crate_b/CHANGELOG.md#unreleased)
        ### Changed
        - `Signature` is a 64 byte 'secure primitive'

        ## [crate_a](crates/crate_a/CHANGELOG.md#unreleased)
        ### Added

        - `InstallAppBundle`
        - `DnaSource`

        ### Removed

        - BREAKING:  `InstallAppDnaPayload`
        - BREAKING: `DnaSource(Path)`


        ## [crate_c](crates/crate_c/CHANGELOG.md#unreleased)
        Awesome changes!

        ### Breaking
        Breaking changes, be careful.

        ## [crate_e](crates/crate_e/CHANGELOG.md#unreleased)
        Awesome changes!

        # [20210304.120604]
        This will include the hdk-0.0.100 release.

        ## [hdk-0.0.100](crates/hdk/CHANGELOG.md#0.0.100)

        ### Changed
        - hdk: fixup the autogenerated hdk documentation.
        "#
    ))
}

/// A workspace with four crates to test changelogs and change detection.
pub fn example_workspace_1<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "crate_a".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "=0.0.0-alpha.1" }"#.to_string(),
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(
                indoc::indoc!(
                    r#"
                    ---
                    semver_increment_mode: minor
                    default_semver_increment_mode: patch
                    ---
                    # Changelog

                    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
                    This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

                    *Note: Versions 0.0.52-alpha2 and older are part belong to previous iterations of the Holochain architecture and are not tracked here.*

                    ## Unreleased

                    ### Added

                    - `InstallAppBundle`
                    - `DnaSource`

                    ### Removed

                    - BREAKING:  `InstallAppDnaPayload`
                    - BREAKING: `DnaSource(Path)`

                    ## 0.0.1

                    This is the first version number for the version of Holochain with a refactored state model (you may see references to it as Holochain RSM).
                    "#
                )
                .to_string(),
            ),
            .. Default::default()
        },
        MockProject {
            name: "crate_b".to_string(),
            version: "0.0.0-alpha.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                r#"
                ---
                ---
                # Changelog
                The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
                This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

                ## [Unreleased]

                ### Changed
                - `Signature` is a 64 byte 'secure primitive'

                [Unreleased]: https://duckduckgo.com/?q=version&t=hd&va=u
                "#
            )),
            .. Default::default()
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

                ### Breaking
                Breaking changes, be careful.

                [Unreleased]: file:///dev/null
                "#
            )),
            .. Default::default()
        },
        MockProject {
            name: "crate_d".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: true,
            ty: workspace_mocker::MockProjectType::Bin,
            .. Default::default()
        },
        MockProject {
            name: "crate_e".to_string(),
            version: "0.0.1-dev.0".to_string(),
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
            .. Default::default()
        },
        MockProject {
            name: "crate_f".to_string(),
            version: "0.2.0".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: Some(indoc::formatdoc!(
                    r#"
                    # Changelog
                    Hello. This crate is releasable.

                    ## Unreleased
                    "#
                )),
            .. Default::default()
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
        This will be removed by aggregation.

        ## [crate_a](crates/crate_a/CHANGELOG.md#unreleased)
        ### Added

        - `InstallAppBundle`

        ## [crate_c](crates/crate_c/CHANGELOG.md#unreleased)
        Awesome changes!

        ### Breaking
        Breaking changes, be careful.

        ## [crate_f](crates/crate_f/CHANGELOG.md#unreleased)
        This will be released in the future.

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
    workspace_mocker.tag("crate_a-0.0.1");
    workspace_mocker.add_or_replace_file(
        "crates/crate_a/README.md",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    // todo: derive the tag from a function
    workspace_mocker.add_or_replace_file(
        "crates/crate_b/README.md",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    workspace_mocker.add_or_replace_file(
        "crates/crate_e/README.md",
        indoc::indoc! {r##"
            # Example

            Some changes
            "##,
        },
    );
    workspace_mocker.commit(None);

    workspace_mocker.add_or_replace_file(
        "crates/crate_f/README.md",
        indoc::indoc! {r#"
            # Example

            Some changes
            "#,
        },
    );
    workspace_mocker.commit(None);

    workspace_mocker.update_lockfile()?;
    workspace_mocker.commit(None);

    Ok(workspace_mocker)
}

/// A workspace to test dependencies and crate sorting.
/// crate_a -> [crate_b, crate_c]
/// crate_b -> []
/// crate_c -> [crate_b]
/// crate_d -> [crate_a]
pub fn example_workspace_2<'a>() -> Fallible<WorkspaceMocker> {
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
            ..Default::default()
        },
        MockProject {
            name: "crate_b".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            ..Default::default()
        },
        MockProject {
            name: "crate_c".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1" }"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Lib,
            changelog: None,
            ..Default::default()
        },
        MockProject {
            name: "crate_d".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_a = { path = "../crate_a", version = "0.0.1" }"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            ..Default::default()
        },
    ];

    WorkspaceMocker::try_new(None, members)
}

/// A workspace that is blocked by an unreleasable dependency.
pub fn example_workspace_3<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "crate_a".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1", optional = true }"#
                    .to_string(),
            ],
            dev_dependencies: vec![
                r#"crate_b = { path = "../crate_b", version = "0.0.1", optional = false }"#
                    .to_string(),
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
            ..Default::default()
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
            ..Default::default()
        },
    ];

    let workspace_mocker = WorkspaceMocker::try_new(None, members)?;

    Ok(workspace_mocker)
}

/// A workspace to test some release blockers
pub fn example_workspace_4<'a>() -> Fallible<WorkspaceMocker> {
    use crate::tests::workspace_mocker::{self, MockProject, WorkspaceMocker};

    let members = vec![
        MockProject {
            name: "wildcard_dependency".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            dev_dependencies: vec![
                r#"wildcard_dependency = {path = ".", version = '*'}"#.to_string()
            ],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            ..Default::default()
        },
        MockProject {
            name: "no_description".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            description: None,
            ..Default::default()
        },
        MockProject {
            name: "no_license".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            license: None,
            ..Default::default()
        },
        MockProject {
            name: "keyword_invalid_char".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            keywords: vec!["1nvalid".to_string()],
            ..Default::default()
        },
        MockProject {
            name: "keyword_toolong".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            keywords: vec!["toolongtoolongtoolongtoolongtoolong".to_string()],
            ..Default::default()
        },
        MockProject {
            name: "keyword_toomany".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"
                # Changelog
                "#
            )),
            keywords: vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
                "four".to_string(),
                "five".to_string(),
                "many".to_string(),
            ],
            ..Default::default()
        },
        MockProject {
            name: "disallowed_semver_increment_mode".to_string(),
            version: "0.0.1".to_string(),
            dependencies: vec![],
            excluded: false,
            ty: workspace_mocker::MockProjectType::Bin,
            changelog: Some(indoc::formatdoc!(
                r#"---
                default_semver_increment_mode: minor
                ---
                # Changelog
                "#
            )),
            keywords: vec![],
            ..Default::default()
        },
    ];

    let workspace_mocker = WorkspaceMocker::try_new(
        Some(indoc::indoc! {r#"
        # Changelog
        "#}),
        members,
    )?;

    Ok(workspace_mocker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example() {
        let workspace_mocker = example_workspace_1().unwrap();
        workspace_mocker.add_or_replace_file(
            "README.md",
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
