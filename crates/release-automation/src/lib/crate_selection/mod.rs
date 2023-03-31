//! Select which crates to include in the release process.

use crate::changelog::{
    self, ChangeT, ChangelogT, ChangelogType, CrateChangelog, WorkspaceChangelog,
};
use crate::common::SemverIncrementMode;
use crate::Fallible;
use cargo::core::Dependency;
use log::{debug, info, trace, warn};

use anyhow::Context;
use anyhow::{anyhow, bail};
use educe::{self, Educe};
use enumflags2::{bitflags, BitFlags};
use linked_hash_map::LinkedHashMap;
use linked_hash_set::LinkedHashSet;
use once_cell::unsync::{Lazy, OnceCell};
use regex::Regex;
use semver::{Comparator, Op, Version, VersionReq};
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::io::Write;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod aliases {
    pub use cargo::core::dependency::DepKind as CargoDepKind;
    pub use cargo::core::package::Package as CargoPackage;
    pub use cargo::core::Workspace as CargoWorkspace;
}
use aliases::*;

fn releaseworkspace_path_only_fmt(
    ws: &&ReleaseWorkspace<'_>,
    f: &mut fmt::Formatter,
) -> fmt::Result {
    write!(f, "{:?}", &ws.root_path)
}

type DependenciesT = LinkedHashMap<String, Vec<cargo::core::Dependency>>;

#[derive(custom_debug::Debug)]
pub struct Crate<'a> {
    package: CargoPackage,
    changelog: Option<ChangelogT<'a, CrateChangelog>>,
    #[debug(with = "releaseworkspace_path_only_fmt")]
    workspace: &'a ReleaseWorkspace<'a>,
    #[debug(skip)]
    dependencies_in_workspace: OnceCell<DependenciesT>,
    #[debug(skip)]
    dependants_in_workspace: OnceCell<Vec<&'a Crate<'a>>>,
}

impl<'a> Crate<'a> {
    /// Instantiate a new Crate with the given CargoPackage.
    pub fn with_cargo_package(
        package: CargoPackage,
        workspace: &'a ReleaseWorkspace<'a>,
    ) -> Fallible<Self> {
        let changelog = {
            let changelog_path = package.root().join("CHANGELOG.md");
            if changelog_path.exists() {
                Some(ChangelogT::<CrateChangelog>::at_path(&changelog_path))
            } else {
                None
            }
        };

        Ok(Self {
            package,
            changelog,
            workspace,
            dependencies_in_workspace: Default::default(),
            dependants_in_workspace: Default::default(),
        })
    }

    /// Return the path of the package's manifest.
    pub fn manifest_path(&self) -> &Path {
        self.package.manifest_path()
    }

    /// Sets the new version for the given crate, updates all workspace dependants,
    /// and returns a refrence to them for post-processing.
    pub fn set_version(
        &'a self,
        dry_run: bool,
        release_version: &semver::Version,
    ) -> Fallible<Vec<&'a Crate<'a>>> {
        debug!(
            "setting version to {} in manifest at {:?}",
            release_version,
            self.manifest_path(),
        );

        let release_version_str = release_version.to_string();

        if !dry_run {
            cargo_next::set_version(self.manifest_path(), release_version_str.as_str())?;
        }

        let dependants = self
            .dependants_in_workspace_filtered(|(_dep_name, deps)| {
                deps.iter().any(|dep| {
                    dep.version_req() != &cargo::util::OptVersionReq::from(VersionReq::STAR)
                })
            })?
            .to_owned();

        for dependant in dependants.iter() {
            dependant.set_dependency_version(&self.name(), &release_version, None, dry_run)?;
        }

        Ok(dependants)
    }

    /// Set a dependency to a specific version
    // Adapted from https://github.com/sunng87/cargo-release/blob/f94938c3f20ef20bc8f971d59de75574a0b18931/src/cargo.rs#L122-L154
    pub fn set_dependency_version(
        &self,
        name: &str,
        version: &Version,
        version_req_override: Option<&VersionReq>,
        dry_run: bool,
    ) -> Fallible<()> {
        debug!(
            "[{}] updating dependency version from dependant {} to version {} in manifest {:?}",
            &self.name(),
            &name,
            &version,
            self.manifest_path(),
        );

        let temp_manifest_path = self
            .manifest_path()
            .parent()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "couldn't get parent of path {}",
                    self.manifest_path().display()
                )
            })?
            .join("Cargo.toml.work");

        {
            let manifest = crate::common::load_from_file(self.manifest_path())?;
            let mut manifest: toml_edit::Document = manifest.parse()?;
            for key in &["dependencies", "dev-dependencies", "build-dependencies"] {
                if manifest.as_table().contains_key(key)
                    && manifest[key]
                        .as_table()
                        .expect("manifest is already verified")
                        .contains_key(name)
                {
                    let existing_version_req = if let Some(Ok(existing_version_req)) =
                        manifest[key][name]["version"].as_str().map(|version| {
                            VersionReq::parse(version).context(anyhow::anyhow!(
                                "parsing version {:?} for dependency {} ",
                                version,
                                self.name()
                            ))
                        }) {
                        existing_version_req
                    } else {
                        debug!(
                            "could not parse {}'s {} version req to string: {:?}",
                            name, key, manifest[key][name]["version"]
                        );

                        continue;
                    };

                    trace!(
                        "version: {:?}, existing version req {:?} in {}",
                        version,
                        existing_version_req,
                        key,
                    );

                    // only set the version if necessary
                    if *key == "dependencies" || existing_version_req != VersionReq::STAR {
                        let final_version_req = if let Some(vr) = version_req_override {
                            vr.clone()
                        } else {
                            let mut version_req = VersionReq::parse(&version.to_string())?;

                            // if the Op of the first Comparator we'll inherit that, the rest will be discarded
                            if let Some(op) = existing_version_req
                                .comparators
                                .first()
                                .map(|comp| {
                                    if comp.op != semver::Op::Wildcard {
                                        Some(comp.op)
                                    } else {
                                        None
                                    }
                                })
                                .flatten()
                            {
                                trace!("overriding first op of {:?} with {:?}", version_req, op);
                                let version_req_clone = version_req.clone();

                                version_req
                                    .comparators
                                    .first_mut()
                                    .ok_or_else(|| anyhow::anyhow!{
                                        "first comparator of version_req {:?} should be accessible",
                                        version_req_clone
                                    })?
                                    .op = op;
                            };

                            version_req
                        };

                        manifest[key][name]["version"] =
                            toml_edit::value(final_version_req.to_string());
                    }
                }
            }

            let mut file_out = std::fs::File::create(&temp_manifest_path)?;
            file_out.write_all(manifest.to_string_in_original_order().as_bytes())?;
        }
        if !dry_run {
            std::fs::rename(temp_manifest_path, self.manifest_path())?;
        }

        Ok(())
    }

    /// Return a reference to the package.
    pub fn package(&self) -> &CargoPackage {
        &self.package
    }

    pub fn state(&self) -> CrateState {
        self.workspace
            .members_states()
            .expect("should be initialised")
            .get(&self.name())
            .expect("should be found")
            .clone()
    }

    /// This crate's name as given in the Cargo.toml file
    pub fn name(&self) -> String {
        self.package.name().to_string()
    }

    /// This crate's current version as given in the Cargo.toml file
    pub fn version(&self) -> Version {
        self.package.version().to_owned()
    }

    /// Return a string in the from of '{package_name}-{package_version}'
    pub fn name_version(&self) -> String {
        format!("{}-{}", self.name(), self.version())
    }

    /// This crate's changelog.
    pub fn changelog(&'a self) -> Option<&ChangelogT<'a, CrateChangelog>> {
        self.changelog.as_ref()
    }

    /// Returns the crates in the same workspace that this crate depends on.
    pub fn dependencies_in_workspace(&'a self) -> Fallible<&'a DependenciesT> {
        self.dependencies_in_workspace.get_or_try_init(|| {
            // LinkedHashSet automatically deduplicates while maintaining the insertion order.
            let mut dependencies = LinkedHashMap::new();
            let ws_members: std::collections::HashMap<_, _> = self
                .workspace
                .members_unsorted()?
                .iter()
                .map(|m| (m.name(), &m.package))
                .collect();

            // This vector is used to implement a depth-first-search to capture all transitive dependencies.
            // Starting with the package in self and traversing down from it.
            let mut queue = vec![&self.package];
            let mut seen = HashSet::new();

            while let Some(package) = queue.pop() {
                for dep in package.dependencies() {
                    let dep_name = dep.package_name().to_string();

                    // todo: write a test-case for this
                    if dep.is_optional() && self.workspace.criteria.exclude_optional_deps {
                        trace!(
                            "[{}] excluding optional dependency '{}'",
                            package.name(),
                            dep_name,
                        );

                        continue;
                    }

                    // only consider workspace members
                    if let Some(dep_package) = ws_members.get(&dep.package_name().to_string()) {
                        // only consider non-star version requirements
                        if dep.specified_req() && dep.version_req().to_string() != "*" {
                            // don't add this package to its own dependencies
                            if dep_package.name() != package.name() {
                                dependencies
                                    .entry(dep_name.clone())
                                    .or_insert_with(|| vec![])
                                    .push(dep.to_owned());

                                if !seen.contains(&dep_name) {
                                    queue.push(dep_package);
                                }
                            } else {
                                warn!(
                                    "encountered dependency cycle: {:?} <-> {:?}",
                                    self.name(),
                                    package.name()
                                );
                            }
                        }
                    }
                }
                seen.insert(package.name().to_string());
            }
            Ok(dependencies)
        })
    }

    /// Returns a reference to all workspace crates that depend on this crate.
    // todo: write a unit test for this
    pub fn dependants_in_workspace(&'a self) -> Fallible<&'a Vec<&'a Crate<'a>>> {
        self.dependants_in_workspace_filtered(|_| true)
    }

    /// Returns a reference to all workspace crates that depend on this crate.
    /// Features filtering by applying a filter function to the dependant's dependencies.
    // todo: write a unit test for this
    pub fn dependants_in_workspace_filtered<F>(
        &'a self,
        filter_fn: F,
    ) -> Fallible<&'a Vec<&'a Crate<'a>>>
    where
        F: Fn(&(&String, &Vec<Dependency>)) -> bool,
        F: Copy,
    {
        self.dependants_in_workspace.get_or_try_init(|| {
            let members_dependants = self.workspace.members()?.iter().try_fold(
                LinkedHashMap::<String, &'a Crate<'a>>::new(),
                |mut acc, member| -> Fallible<_> {
                    if member
                        .dependencies_in_workspace()?
                        .iter()
                        // FIXME: applying the filter here is incorrect, because
                        // it persists the return value for the first call and
                        // returns that for every subsequent call, regardless of
                        // the filter function that's passed
                        .filter(filter_fn)
                        .map(|(dep_name, _)| dep_name)
                        .collect::<LinkedHashSet<_>>()
                        .contains(&self.name())
                    {
                        acc.insert(member.name(), *member);
                    };

                    Ok(acc)
                },
            )?;

            Ok(members_dependants.values().cloned().collect())
        })
    }

    pub fn root(&self) -> &Path {
        self.package.root()
    }

    pub fn workspace(&self) -> &'a ReleaseWorkspace<'a> {
        self.workspace
    }
}

type MemberStates = LinkedHashMap<String, CrateState>;

#[derive(custom_debug::Debug)]
pub struct ReleaseWorkspace<'a> {
    root_path: PathBuf,
    criteria: SelectionCriteria,
    git_config_name: String,
    git_config_email: String,

    changelog: Option<ChangelogT<'a, WorkspaceChangelog>>,

    #[debug(skip)]
    cargo_config: cargo::util::config::Config,
    cargo_workspace: OnceCell<CargoWorkspace<'a>>,
    members_unsorted: OnceCell<Vec<Crate<'a>>>,
    members_sorted: OnceCell<Vec<&'a Crate<'a>>>,
    members_matched: OnceCell<Vec<&'a Crate<'a>>>,
    members_states: OnceCell<MemberStates>,
    #[debug(skip)]
    git_repo: git2::Repository,
}

/// Configuration criteria for the crate selection.
#[derive(Educe, Debug)]
#[educe(Default)]
pub struct SelectionCriteria {
    #[educe(Default(expression = r#"fancy_regex::Regex::new(".*").expect("matching anything is valid")"#r))]
    pub match_filter: fancy_regex::Regex,
    pub enforced_version_reqs: Vec<semver::VersionReq>,
    pub disallowed_version_reqs: Vec<semver::VersionReq>,
    pub allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,
    pub allowed_selection_blockers: BitFlags<CrateStateFlags>,
    pub allowed_semver_increment_modes: Option<HashSet<SemverIncrementMode>>,
    pub exclude_optional_deps: bool,
}

/// Defines detailed crate's state in terms of the release process.
#[bitflags]
#[repr(u32)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CrateStateFlags {
    /// matches a package filter
    Matched,
    /// in the dependency tree of a matched package
    IsWorkspaceDependency,
    /// in the dev-dependency tree of a matched package
    IsWorkspaceDevDependency,
    /// has changed since previous release if any
    HasPreviousRelease,
    /// Has no previous release
    NoPreviousRelease,
    /// Has a previous release but its tag is missing
    MissingReleaseTag,
    /// has changed since previous release
    ChangedSincePreviousRelease,
    /// At least one dependency is marked as changed.
    DependencyChanged,

    /// has `unreleasable: true` set in changelog
    MissingChangelog,
    MissingReadme,
    UnreleasableViaChangelogFrontmatter,
    EnforcedVersionReqViolated,
    DisallowedVersionReqViolated,
    /// Has no description in the Cargo.toml
    MissingDescription,
    /// Has no license in the Cargo.toml
    MissingLicense,
    /// Has a dependency that contains '*'
    HasWildcardDependency,
    /// Has a dev-dependency that contains '*'
    HasWildcardDevDependency,
    /// One of the manifest keywords is too long
    ManifestKeywordExceeds20Chars,
    ManifestKeywordContainsInvalidChar,
    ManifestKeywordsMoreThan5,
    AllowedSemverIncrementModeViolated,
}

/// Defines the meta states that can be derived from the more detailed `CrateStateFlags`.
#[bitflags]
#[repr(u16)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MetaCrateStateFlags {
    Allowed,
    Blocked,
    Changed,
    Selected,
}

impl CrateStateFlags {
    pub fn empty_set() -> BitFlags<Self> {
        BitFlags::empty()
    }
}

/// Implements the logic for determining a crate's starte in terms of the release process.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CrateState {
    flags: BitFlags<CrateStateFlags>,
    meta_flags: BitFlags<MetaCrateStateFlags>,

    allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,
    allowed_selection_blockers: BitFlags<CrateStateFlags>,
}

impl CrateState {
    pub const BLOCKING_STATES: BitFlags<CrateStateFlags> = enumflags2::make_bitflags!(
        CrateStateFlags::{MissingChangelog
            | MissingReadme
            | UnreleasableViaChangelogFrontmatter
            | DisallowedVersionReqViolated
            | EnforcedVersionReqViolated
            | MissingDescription
            | MissingLicense
            | HasWildcardDependency
            | ManifestKeywordExceeds20Chars
            | ManifestKeywordContainsInvalidChar
            | ManifestKeywordsMoreThan5
            | AllowedSemverIncrementModeViolated
    });

    pub fn new(
        flags: BitFlags<CrateStateFlags>,
        allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,
        allowed_selection_blockers: BitFlags<CrateStateFlags>,
    ) -> Self {
        let mut new = Self {
            flags,
            meta_flags: Default::default(),
            allowed_dev_dependency_blockers,
            allowed_selection_blockers,
        };
        new.update_meta_flags();
        new
    }

    pub fn contains(&self, flag: CrateStateFlags) -> bool {
        self.flags.contains(flag)
    }

    pub fn merge(&mut self, other: Self) {
        self.flags.extend(other.flags.iter());
        self.update_meta_flags();
    }

    pub fn insert(&mut self, flag: CrateStateFlags) {
        self.flags.insert(flag);
        self.update_meta_flags();
    }

    pub fn is_matched(&self) -> bool {
        self.flags.contains(CrateStateFlags::Matched)
    }

    pub fn is_dependency(&self) -> bool {
        self.flags.contains(CrateStateFlags::IsWorkspaceDependency)
    }

    pub fn is_dev_dependency(&self) -> bool {
        self.flags
            .contains(CrateStateFlags::IsWorkspaceDevDependency)
    }

    fn update_meta_flags(&mut self) {
        if self.changed() {
            self.meta_flags.insert(MetaCrateStateFlags::Changed);
        } else {
            self.meta_flags.remove(MetaCrateStateFlags::Changed);
        }

        if !self.blocked_by().is_empty() {
            self.meta_flags.insert(MetaCrateStateFlags::Blocked);
        } else {
            self.meta_flags.remove(MetaCrateStateFlags::Blocked);
        }

        if self.allowed() {
            self.meta_flags.insert(MetaCrateStateFlags::Allowed);
        } else {
            self.meta_flags.remove(MetaCrateStateFlags::Allowed);
        }

        if self.selected() {
            self.meta_flags.insert(MetaCrateStateFlags::Selected);
        } else {
            self.meta_flags.remove(MetaCrateStateFlags::Selected);
        }
    }

    fn blocked_by(&self) -> BitFlags<CrateStateFlags> {
        Self::BLOCKING_STATES.intersection_c(self.flags)
    }

    fn disallowed_blockers(&self) -> BitFlags<CrateStateFlags> {
        let mut blocking_flags = self.blocked_by();

        match (self.is_matched(), self.is_dev_dependency()) {
            (true, _) => blocking_flags.remove(self.allowed_selection_blockers),
            (_, true) => blocking_flags.remove(self.allowed_dev_dependency_blockers),
            _ => {}
        }

        blocking_flags
    }

    fn blocked(&self) -> bool {
        !self.blocked_by().is_empty()
    }

    fn allowed(&self) -> bool {
        self.disallowed_blockers().is_empty()
    }

    /// There are changes to be released.
    pub fn changed(&self) -> bool {
        self.flags.contains(CrateStateFlags::NoPreviousRelease)
            || self.flags.contains(CrateStateFlags::MissingReleaseTag)
            || self
                .flags
                .contains(CrateStateFlags::ChangedSincePreviousRelease)
    }

    /// At least one dependency is marked as changed.
    pub fn dependency_changed(&self) -> bool {
        self.flags.contains(CrateStateFlags::DependencyChanged)
    }

    /// There are changes to be released since the previous release
    pub fn changed_since_previous_release(&self) -> bool {
        self.flags
            .contains(CrateStateFlags::ChangedSincePreviousRelease)
    }

    /// Has a prevoius release.
    pub fn has_previous_release(&self) -> bool {
        self.flags.contains(CrateStateFlags::HasPreviousRelease)
    }

    /// Has been matched explicitly or as a consequence of a dependency.
    pub fn selected(&self) -> bool {
        self.is_matched() || self.is_dependency() || self.is_dev_dependency()
    }

    /// Will be included in the release
    pub fn release_selection(&self) -> bool {
        !self.blocked() && (self.changed() || self.dependency_changed()) && self.selected()
    }

    /// Returns a formatted string with an overview of crates and their states.
    pub fn format_crates_states<'cs, CS>(
        states: CS,
        title: &str,
        show_blocking: bool,
        show_flags: bool,
        show_meta: bool,
    ) -> String
    where
        CS: std::iter::IntoIterator<Item = &'cs (String, CrateState)>,
    {
        let mut states_shown = if show_blocking || show_flags || show_meta {
            "Showing states: "
        } else {
            ""
        }
        .to_string();
        if show_blocking {
            states_shown += "* Disallowed Blocking "
        }
        if show_flags {
            states_shown += "* Flags "
        }
        if show_meta {
            states_shown += "* Meta"
        }
        if !states_shown.is_empty() {
            states_shown += "\n";
        }

        let mut msg = format!("\n{0:-<80}\n{1}\n{2}", "", title.to_owned(), states_shown,);
        for (name, state) in states {
            msg += &format!("{empty:-<80}\n{name:<30}", empty = "", name = name);
            if show_blocking {
                msg += &format!(
                    "{blocking_flags:?}\n{empty:<30}",
                    empty = "",
                    blocking_flags = state.disallowed_blockers().iter().collect::<Vec<_>>(),
                );
            }

            if show_flags {
                msg += &format!(
                    "{flags:?}\n{empty:<30}",
                    empty = "",
                    flags = state.flags.iter().collect::<Vec<_>>(),
                );
            };

            if show_meta {
                msg += &format!(
                    "{meta_flags:?}",
                    meta_flags = state.meta_flags.iter().collect::<Vec<_>>(),
                );
            };

            msg += &"\n".to_string();
        }

        msg
    }
}

impl<'a> ReleaseWorkspace<'a> {
    const README_FILENAME: &'a str = "README.md";
    const GIT_CONFIG_NAME: &'a str = "Holochain Core Dev Team";
    const GIT_CONFIG_EMAIL: &'a str = "devcore@holochain.org";

    pub fn try_new_with_criteria(
        root_path: PathBuf,
        criteria: SelectionCriteria,
    ) -> Fallible<ReleaseWorkspace<'a>> {
        Ok(Self {
            criteria,
            ..Self::try_new(root_path)?
        })
    }

    /// Reset all cached state which will cause a reload the next time any method is called.
    pub fn reset_state(&mut self) {
        self.cargo_workspace = Default::default();
        self.cargo_workspace = Default::default();
        self.members_unsorted = Default::default();
        self.members_sorted = Default::default();
        self.members_states = Default::default();
    }

    pub fn try_new(root_path: PathBuf) -> Fallible<ReleaseWorkspace<'a>> {
        let changelog = {
            let changelog_path = root_path.join("CHANGELOG.md");
            if changelog_path.exists() {
                Some(ChangelogT::<WorkspaceChangelog>::at_path(&changelog_path))
            } else {
                None
            }
        };

        let new = Self {
            // initialised: false,
            git_repo: git2::Repository::open(&root_path)?,

            git_config_name: Self::GIT_CONFIG_NAME.to_string(),
            git_config_email: Self::GIT_CONFIG_EMAIL.to_string(),

            root_path,
            criteria: Default::default(),
            changelog,
            cargo_config: cargo::util::config::Config::default()?,

            cargo_workspace: Default::default(),
            members_unsorted: Default::default(),
            members_sorted: Default::default(),
            members_matched: Default::default(),
            members_states: Default::default(),
        };

        // todo(optimization): eagerly ensure that the workspace is valid, but the following fails lifetime checks
        // let _ = new.cargo_workspace()?;

        Ok(new)
    }

    fn members_states(&'a self) -> Fallible<&MemberStates> {
        self.members_states.get_or_try_init(|| {
            let mut members_states = MemberStates::new();

            let criteria = &self.criteria;
            let initial_state = CrateState {
                allowed_dev_dependency_blockers: criteria.allowed_dev_dependency_blockers,
                allowed_selection_blockers: criteria.allowed_selection_blockers,

                ..Default::default()
            };

            let keyword_validation_re = Regex::new("^[a-zA-Z][a-zA-Z_\\-0-9]+$").unwrap();

            for member in self.members()? {

                // helper macros to access the desired state
                macro_rules! get_state {
                    ( $i:expr ) => {
                        members_states.entry($i).or_insert(initial_state.clone())
                    };
                }
                macro_rules! insert_state {
                    ( $flag:expr ) => {
                        insert_state!($flag, member.name())
                    };
                    ( $flag:expr, $i:expr ) => {
                        get_state!($i).insert($flag)
                    };
                }

                // manifest metadata validation
                {
                    let metadata = member.package().manifest().metadata();
                    if !(metadata.license.is_some() || metadata.license_file.is_some()) {
                        insert_state!(CrateStateFlags::MissingLicense);
                    }

                    if metadata.description.is_none() {
                        insert_state!(CrateStateFlags::MissingDescription);
                    }

                    // see https://doc.rust-lang.org/cargo/reference/manifest.html?highlight=keywords#the-keywords-field
                    // Note: crates.io has a maximum of 5 keywords. Each keyword must be ASCII text, start with a letter, and only contain letters, numbers, _ or -, and have at most 20 characters.
                    if metadata.keywords.iter().any(|keyword| keyword.len() > 20) {
                        insert_state!(CrateStateFlags::ManifestKeywordExceeds20Chars);
                    }
                    if metadata.keywords.iter().any(|keyword| !keyword_validation_re.is_match(keyword)) {
                        insert_state!(CrateStateFlags::ManifestKeywordContainsInvalidChar);
                    }
                    if metadata.keywords.len() > 5 {
                        insert_state!(CrateStateFlags::ManifestKeywordsMoreThan5);
                    }
                }

                // regex matching state
                if criteria.match_filter.is_match(&member.name())? {
                    insert_state!(CrateStateFlags::Matched);
                }

                // version requirements
                {
                    let version = member.version();

                    criteria
                        .enforced_version_reqs
                        .iter()
                        .filter(|enforced_version_req| !enforced_version_req.matches(&version))
                        .take(1)
                        .for_each(|enforced_version_req| {
                            warn!(
                                "'{}' version '{}' doesn't meet the enforced requirement '{}'",
                                member.name(),
                                version,
                                enforced_version_req
                            );
                            insert_state!(CrateStateFlags::EnforcedVersionReqViolated);
                        });

                    criteria
                        .disallowed_version_reqs
                        .iter()
                        .filter(|disallowed_version_req| disallowed_version_req.matches(&version))
                        .take(1)
                        .for_each(|disallowed_version_req| {
                            warn!(
                                "'{}' version '{}' matches the disallowed requirement '{}'",
                                member.name(),
                                version,
                                disallowed_version_req
                            );
                            insert_state!(CrateStateFlags::DisallowedVersionReqViolated);
                        });

                    if !std::path::Path::new(&member.root().join(Self::README_FILENAME)).exists() {
                        insert_state!(CrateStateFlags::MissingReadme);
                    }

                    // change related state
                    match member.changelog() {
                        None => {
                            warn!("'{}' is missing the changelog", member.name());
                            insert_state!(CrateStateFlags::MissingChangelog);
                        }

                        Some(changelog) => {
                            if let Some(front_matter) = changelog.front_matter().context(
                                format!("when parsing front matter of crate '{}'", member.name()),
                            )? {
                                if front_matter.unreleasable() {
                                    warn!("'{}' has unreleasable defined via the changelog frontmatter", member.name());
                                    insert_state!(
                                        CrateStateFlags::UnreleasableViaChangelogFrontmatter
                                    );
                                }
                            }

                            if let Some(changelog::ReleaseChange::CrateReleaseChange(previous_release_version)) =
                                changelog
                                    .changes()
                                    .ok()
                                    .iter()
                                    .flatten()
                                    .filter_map(|r| {
                                        if let ChangeT::Release(r) = r {
                                            Some(r)
                                        } else {
                                            None
                                        }
                                    })
                                    .take(1)
                                    .next()
                            {

                                // todo: derive the tagname from a function?
                                // lookup the git tag for the previous release
                                let maybe_git_tag =
                                        git_lookup_tag(&self.git_repo, format!("{}-{}", &member.name(), previous_release_version).as_str());

                                log::debug!("[{}] previous release: {}, previous git tag {:?}", member.name(), previous_release_version, maybe_git_tag);

                                if let Some(git_tag) = maybe_git_tag {

                                    insert_state!(CrateStateFlags::HasPreviousRelease);

                                    // todo: make comparison ref configurable
                                    let changed_files = changed_files(member.package.root(), &git_tag, "HEAD")?;
                                    if !changed_files.is_empty()
                                    {
                                        debug!("[{}] changed files since {git_tag}: {changed_files:?}", member.name());
                                        insert_state!(CrateStateFlags::ChangedSincePreviousRelease)
                                    }
                                } else {
                                    insert_state!(CrateStateFlags::MissingReleaseTag);
                                }
                            } else {
                                    insert_state!(CrateStateFlags::NoPreviousRelease);
                            }
                        }
                    }

                    // semver_increment_mode checks
                    if let Some(allowed_semver_increment_modes) = &self.criteria.allowed_semver_increment_modes {
                        let effective_semver_increment_mode  = member
                            .changelog()
                            .map(|cl| cl.front_matter().ok())
                            .flatten()
                            .flatten()
                            .map(|fm| fm.semver_increment_mode())
                            .unwrap_or_default();


                        if !allowed_semver_increment_modes.contains(&effective_semver_increment_mode) {
                            debug!("Blocking {} due to {:?} with mode: {effective_semver_increment_mode:?}", member.name(), CrateStateFlags::AllowedSemverIncrementModeViolated);
                            insert_state!(CrateStateFlags::AllowedSemverIncrementModeViolated);
                        }
                    }
                }

                {
                    // dependency state
                    // only dependencies of explicitly matched packages are considered here.
                    // this detects changes in the transitive dependency chain by two mechanisms
                    // 1. the loop we're in iterates over the result of `ReleaseWorkspace::members`,
                    //    which orders the members according to the workspace dependency trees from leafs to roots.
                    //    this ensures that the states of a member's transitive dependencies have been evaluated by the time *it* is evaluated.
                    // 2. the `member.dependencies_in_workspace()` yields transitive results.
                    if get_state!(member.name()).is_matched()
                    {
                        for (_, deps) in member.dependencies_in_workspace()? {
                            for dep in deps {
                                insert_state!(
                                    match dep.kind() {
                                        CargoDepKind::Development => CrateStateFlags::IsWorkspaceDevDependency,
                                        _ => CrateStateFlags::IsWorkspaceDependency,
                                    },
                                    dep.package_name().to_string()
                                );
                            }
                        }

                        for dep in member.package().dependencies() {
                            if dep.version_req().to_string().contains('*') {
                                insert_state!(match dep.kind() {
                                    CargoDepKind::Normal | CargoDepKind::Build => CrateStateFlags::HasWildcardDependency,
                                    CargoDepKind::Development => CrateStateFlags::HasWildcardDevDependency,
                                });
                            }
                        }
                    }

                    // set DependencyChanged in dependants if this crate changed
                    if get_state!(member.name()).changed() {
                        for dependant in member.dependants_in_workspace()? {
                            insert_state!(
                                CrateStateFlags::DependencyChanged,
                                dependant.name()
                            );
                        }
                    }
                }

            }

            Ok(members_states)
        })
    }

    fn cargo_workspace(&'a self) -> Fallible<&'a CargoWorkspace> {
        self.cargo_workspace.get_or_try_init(|| {
            CargoWorkspace::new(&self.root_path.join("Cargo.toml"), &self.cargo_config)
        })
    }

    /// Returns the crates that are going to be processed for release.
    pub fn release_selection(&'a self) -> Fallible<Vec<&'a Crate>> {
        let members = self.members()?;

        let all_crates_states_iter = members.iter().map(|member| (member.name(), member.state()));
        let all_crates_states = all_crates_states_iter.clone().collect::<Vec<_>>();
        trace!(
            "{}",
            CrateState::format_crates_states(&all_crates_states, "ALL CRATES", true, true, true,)
        );
        let blocked_crates_states = all_crates_states_iter
            .clone()
            .filter(|(_, state)| state.selected() && !state.allowed())
            .collect::<Vec<_>>();

        // indicate an error if any unreleasable crates block the release
        if !blocked_crates_states.is_empty() {
            bail!(
                "the following crates are blocked but required for the release: \n{}",
                CrateState::format_crates_states(
                    &blocked_crates_states,
                    "DISALLOWED BLOCKING CRATES",
                    true,
                    false,
                    false,
                )
            )
        }

        let release_selection = members
            .iter()
            .filter(|member| {
                let release = member.state().release_selection();

                trace!(
                    "{} release indicator: {}, blocked: {:?}, state: {:#?}",
                    member.name(),
                    release,
                    member.state().blocked(),
                    member.state(),
                );

                release
            })
            .cloned()
            .collect::<Vec<_>>();

        Ok(release_selection)
    }

    fn members_unsorted(&'a self) -> Fallible<&'a Vec<Crate<'a>>> {
        self.members_unsorted.get_or_try_init(|| {
            let mut members = vec![];

            for package in self.cargo_workspace()?.members() {
                members.push(Crate::with_cargo_package(package.to_owned(), self)?);
            }

            Ok(members)
        })
    }

    /// Return all member crates matched by `SelectionCriteria::match_filter`
    pub fn members_matched(&'a self) -> Fallible<&'a Vec<&'a Crate<'a>>> {
        self.members_matched.get_or_try_init(|| {
            let states = self.members_states()?;

            self.members().map(|members| {
                members
                    .into_iter()
                    .filter(|crt| {
                        states
                            .get(&crt.name())
                            .cloned()
                            .unwrap_or_else(|| {
                                warn!(
                                    "cannot get CrateState for {}, using default state",
                                    crt.name()
                                );
                                CrateState::default()
                            })
                            .contains(CrateStateFlags::Matched)
                    })
                    .map(|crt| *crt)
                    .collect::<Vec<_>>()
            })
        })
    }

    /// Returns all non-excluded workspace members.
    /// Members are sorted according to their dependency tree from most independent to most dependent.
    pub fn members(&'a self) -> Fallible<&'a Vec<&'a Crate<'a>>> {
        self.members_sorted.get_or_try_init(|| -> Fallible<_> {
            let mut members = self
                .members_unsorted()?
                .iter()
                .enumerate()
                .collect::<Vec<_>>();

            let workspace_dependencies = self.members_unsorted()?.iter().try_fold(
                LinkedHashMap::<String, LinkedHashSet<String>>::new(),
                |mut acc, elem| -> Fallible<_> {
                    acc.insert(
                        elem.name(),
                        elem.dependencies_in_workspace()?
                            .into_iter()
                            .filter_map(|(dep_name, deps)| {
                                deps.into_iter()
                                    .find(|dep| {
                                        dep.specified_req() && dep.version_req().to_string() != "*"
                                    })
                                    .map(|_| dep_name.clone())
                            })
                            .collect(),
                    );

                    Ok(acc)
                },
            )?;

            // ensure members are ordered respecting their dependency tree
            members.sort_unstable_by(move |(a_i, a), (b_i, b)| {
                use std::cmp::Ordering::{Equal, Greater, Less};

                let a_deps = workspace_dependencies
                    .get(&a.name())
                    .unwrap_or_else(|| panic!("dependencies for {} not found", a.name()));
                let b_deps = workspace_dependencies
                    .get(&b.name())
                    .unwrap_or_else(|| panic!("dependencies for {} not found", b.name()));

                // understand whether one is a direct dependency of the other
                let comparison = (a_deps.contains(&b.name()), b_deps.contains(&a.name()));
                let result = match comparison {
                    (true, true) => {
                        panic!("cyclic dependency between {} and {}", a.name(), b.name())
                    }
                    (true, false) => Greater,
                    (false, true) => Less,
                    (false, false) => a_i.cmp(b_i),
                };

                trace!(
                    "comparing \n{} ({:?}) with \n{} ({:?})\n{:?} => {:?}",
                    a.name(),
                    a_deps,
                    b.name(),
                    b_deps,
                    comparison,
                    result
                );
                result
            });

            Ok(members.into_iter().map(|(_, member)| member).collect())
        })
    }

    /// Return the root path of the workspace.
    pub fn root(&'a self) -> &Path {
        &self.root_path
    }

    pub fn git_repo(&'a self) -> &git2::Repository {
        &self.git_repo
    }

    /// Tries to resolve the git HEAD to its corresponding branch.
    pub fn git_head_branch(&'a self) -> Fallible<(git2::Branch, git2::BranchType)> {
        for branch in self.git_repo.branches(None)? {
            let branch = branch?;
            if branch.0.is_head() {
                return Ok(branch);
            }
        }

        bail!("head branch not found")
    }

    /// Calls Self::git_head_branch and tries to resolve its name to String.
    pub fn git_head_branch_name(&'a self) -> Fallible<String> {
        self.git_head_branch().map(|(branch, _)| {
            branch
                .name()?
                .map(String::from)
                .ok_or_else(|| anyhow::anyhow!("the current git branch has no name"))
        })?
    }

    /// Creates a git branch with the given name off of the current HEAD, optionally overwriting the branch if it exists.
    pub fn git_checkout_branch(&'a self, name: &str, force: bool) -> Fallible<git2::Branch> {
        let head_commit = self.git_repo.head()?.peel_to_commit()?;

        let new_branch = self.git_repo.branch(name, &head_commit, force)?;

        let (object, reference) = self.git_repo.revparse_ext(name)?;

        self.git_repo.checkout_tree(&object, None)?;

        let reference_name = reference
            .ok_or_else(|| anyhow::anyhow!("couldn't parse branch new branch to reference"))?
            .name()
            .ok_or_else(|| anyhow::anyhow!("couldn't get reference name"))?
            .to_owned();

        self.git_repo.set_head(&reference_name)?;

        Ok(new_branch)
    }

    /// Creates a new git branch with the given name off of the current HEAD.
    pub fn git_checkout_new_branch(&'a self, name: &str) -> Fallible<git2::Branch> {
        self.git_checkout_branch(name, false)
    }

    // todo: make this configurable?
    fn git_signature(&self) -> Fallible<git2::Signature> {
        Ok(git2::Signature::now(
            &self.git_config_name,
            &self.git_config_email,
        )?)
    }

    /// Add the given files and create a commit.
    pub fn git_add_all_and_commit(
        &'a self,
        msg: &str,
        path_filter: Option<&mut git2::IndexMatchedPath<'_>>,
    ) -> Fallible<git2::Oid> {
        let repo = self.git_repo();

        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, path_filter)?;
        index.write()?;

        let tree_id = repo.index()?.write_tree()?;
        let sig = self.git_signature()?;
        let mut parents = Vec::new();

        if let Some(parent) = repo.head().ok().map(|h| h.target().unwrap()) {
            parents.push(repo.find_commit(parent)?)
        }
        let parents = parents.iter().collect::<Vec<_>>();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            msg,
            &repo.find_tree(tree_id)?,
            &parents,
        )
        .map_err(anyhow::Error::from)
    }

    /// Create a new git tag from HEAD
    pub fn git_tag(&self, name: &str, force: bool) -> Fallible<git2::Oid> {
        let head = self
            .git_repo
            .head()?
            .target()
            .ok_or_else(|| anyhow::anyhow!("repo head doesn't have a target"))?;
        self.git_repo
            .tag(
                name,
                &self.git_repo.find_object(head, None)?,
                &self.git_signature()?,
                &format!("tag for release {}", name),
                force,
            )
            .context(format!("creating tag '{}'", name))
    }

    pub fn changelog(&'a self) -> Option<&'a ChangelogT<'a, WorkspaceChangelog>> {
        self.changelog.as_ref()
    }

    pub fn update_lockfile<T>(&'a self, dry_run: bool, additional_manifests: T) -> Fallible<()>
    where
        T: Iterator<Item = &'a str>,
        T: Clone,
    {
        for args in [
            vec![
                vec!["fetch", "--verbose", "--manifest-path", "Cargo.toml"],
                [
                    vec!["update", "--workspace", "--offline", "--verbose"],
                    if dry_run { vec!["--dry-run"] } else { vec![] },
                ]
                .concat(),
            ],
            additional_manifests
                .clone()
                .map(|mp| {
                    vec![
                        vec!["fetch", "--verbose", "--manifest-path", mp],
                        vec![
                            vec![
                                "update",
                                "--workspace",
                                "--offline",
                                "--verbose",
                                "--manifest-path",
                                mp,
                            ],
                            if dry_run { vec!["--dry-run"] } else { vec![] },
                        ]
                        .concat(),
                    ]
                })
                .collect::<Vec<Vec<_>>>()
                .concat(),
        ]
        .concat()
        {
            let mut cmd = std::process::Command::new("cargo");
            cmd.current_dir(self.root()).args(args);
            debug!("running command: {:?}", cmd);

            if !dry_run {
                let mut cmd = cmd.spawn()?;
                let cmd_status = cmd.wait()?;
                if !cmd_status.success() {
                    bail!("running {:?} failed: \n{:?}", cmd, cmd.stderr);
                }
            }
        }

        Ok(())
    }

    pub fn cargo_check<T>(&'a self, offline: bool, additional_manifests: T) -> Fallible<()>
    where
        T: Iterator<Item = &'a str>,
    {
        for args in [
            vec![vec![
                vec![
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--release",
                ],
                if offline { vec!["--offline"] } else { vec![] },
            ]
            .concat()],
            additional_manifests
                .map(|mp| -> Vec<&str> {
                    vec![
                        vec![
                            "check",
                            "--all-targets",
                            "--all-features",
                            "--release",
                            "--manifest-path",
                            mp,
                        ],
                        if offline { vec!["--offline"] } else { vec![] },
                    ]
                    .concat()
                })
                .collect::<Vec<_>>(),
        ]
        .concat()
        {
            let mut cmd = std::process::Command::new("cargo");
            cmd.current_dir(self.root()).args(args);
            debug!("running command: {:?}", cmd);

            let mut cmd = cmd.spawn()?;
            let cmd_status = cmd.wait()?;
            if !cmd_status.success() {
                bail!("running {:?} failed: \n{:?}", cmd, cmd.stderr);
            }
        }

        Ok(())
    }
}

/// Use the `git` shell command to detect changed files in the given directory between the given revisions.
///
/// Inspired by: https://github.com/sunng87/cargo-release/blob/master/src/git.rs
fn changed_files(dir: &Path, from_rev: &str, to_rev: &str) -> Fallible<Vec<PathBuf>> {
    use bstr::ByteSlice;

    let output = Command::new("git")
        .arg("diff")
        .arg(&format!("{}..{}", from_rev, to_rev))
        .arg("--name-only")
        .arg("--exit-code")
        .arg(".")
        .current_dir(dir)
        .output()?;

    match output.status.code() {
        Some(0) => Ok(Vec::new()),
        Some(1) => {
            let paths = output
                .stdout
                .lines()
                .map(|l| dir.join(l.to_path_lossy()))
                .collect();
            Ok(paths)
        }
        code => Err(anyhow!("git exited with code: {:?}", code)),
    }
}

/// Find a git tag in a repository
// todo: refactor into common place module
pub fn git_lookup_tag(git_repo: &git2::Repository, tag_name: &str) -> Option<String> {
    let tag = git_repo
        .revparse_single(tag_name)
        .ok()
        .map(|obj| obj.as_tag().cloned())
        .flatten()
        .map(|tag| tag.name().unwrap_or_default().to_owned());

    trace!("looking up tag '{}' -> {:?}", tag_name, tag);

    tag
}

// we shouldn't need this check but so far the failing case hasn't been reproduced in a test.
pub fn ensure_release_order_consistency<'a>(
    crates: &[&'a Crate<'a>],
) -> Fallible<LinkedHashSet<String>> {
    crates
        .iter()
        .try_fold(LinkedHashSet::new(), |mut acc, cur| {
            let wrong_order_deps = cur
                .dependencies_in_workspace()?
                .iter()
                .filter_map(|(dep_package_name, _)| {
                    if crates
                        .iter()
                        .any(|selected| &selected.name() == dep_package_name)
                        && !acc.contains(dep_package_name)
                    {
                        Some(dep_package_name.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if !wrong_order_deps.is_empty() {
                bail!(
                    "{} depends on crates that are ordered after it: {:#?}. this is a bug.",
                    cur.name(),
                    wrong_order_deps
                );
            }

            acc.insert(cur.name());

            Ok(acc)
        })
}

#[cfg(test)]
pub mod tests;
