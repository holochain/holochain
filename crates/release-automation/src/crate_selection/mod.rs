//! Select which crates to include in the release process.

use crate::changelog::{
    self, ChangeT, ChangelogT, ChangelogType, CrateChangelog, WorkspaceChangelog,
};
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
use semver::Version;
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) mod aliases {
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

pub(crate) type DependenciesT = LinkedHashMap<String, LinkedHashSet<cargo::core::Dependency>>;

#[derive(custom_debug::Debug)]
pub(crate) struct Crate<'a> {
    package: CargoPackage,
    changelog: Option<ChangelogT<'a, CrateChangelog>>,
    #[debug(with = "releaseworkspace_path_only_fmt")]
    workspace: &'a ReleaseWorkspace<'a>,
    #[debug(skip)]
    dependencies: OnceCell<DependenciesT>,
    #[debug(skip)]
    dependants_in_workspace: OnceCell<Vec<&'a Crate<'a>>>,
}

impl<'a> Crate<'a> {
    /// Instantiate a new Crate with the given CargoPackage.
    pub(crate) fn with_cargo_package(
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
            dependencies: Default::default(),
            dependants_in_workspace: Default::default(),
        })
    }

    /// Return the path of the package's manifest.
    pub(crate) fn manifest_path(&self) -> &Path {
        self.package.manifest_path()
    }

    /// Return a reference to the package.
    pub(crate) fn package(&self) -> &CargoPackage {
        &self.package
    }

    pub(crate) fn state(&self) -> CrateState {
        self.workspace
            .members_states()
            .expect("should be initialised")
            .get(&self.name())
            .expect("should be found")
            .clone()
    }

    /// This crate's name as given in the Cargo.toml file
    pub(crate) fn name(&self) -> String {
        self.package.name().to_string()
    }

    /// This crate's current version as given in the Cargo.toml file
    pub(crate) fn version(&self) -> Version {
        self.package.version().to_owned()
    }

    /// Return a string in the from of '{package_name}-{package_version}'
    pub(crate) fn name_version(&self) -> String {
        format!("{}-{}", self.name(), self.version())
    }

    /// This crate's changelog.
    pub(crate) fn changelog(&'a self) -> Option<&ChangelogT<'a, CrateChangelog>> {
        self.changelog.as_ref()
    }

    /// Returns the crates in the same workspace that this crate depends on.
    pub(crate) fn dependencies(&'a self) -> Fallible<&'a DependenciesT> {
        self.dependencies.get_or_try_init(|| {
            // LinkedHashSet automatically deduplicates while maintaining the insertion order.
            let mut dependencies = DependenciesT::default();

            // This vector is used to implement a depth-first-search to capture all transitive dependencies.
            // Starting with the package in self and traversing down from it.
            let mut queue = vec![&self.package];
            let mut visited = HashSet::new();

            let ws_members = self.workspace.members_unsorted_mapped()?;

            while let Some(package) = queue.pop() {
                visited.insert(package.name());
                for dep in package.dependencies() {
                    if dep.source_id().is_path() {
                        let dep_name = dep.package_name().to_string();

                        // todo(backlog): enable the optional dependency if a feature requested it

                        // todo: write a test-case for this
                        if dep.is_optional() && self.workspace.criteria.exclude_optional_deps {
                            trace!(
                                "[{}] excluding optional dependency '{}'",
                                package.name(),
                                dep_name,
                            );

                            continue;
                        }

                        // ignore dev-dependencies that have no version specified, as they will be ignored by `cargo publish` as well
                        if let CargoDepKind::Development = dep.kind() {
                            if !dep.specified_req() {
                                warn!(
                                    "[{}] excluding unversioned dev-dependency '{}'",
                                    package.name(),
                                    dep_name
                                );

                                continue;
                            }
                        }

                        dependencies
                            .entry(dep.package_name().to_string())
                            .or_insert_with(|| LinkedHashSet::<Dependency>::default())
                            .insert(dep.to_owned());

                        if dep.source_id().local_path().is_some() {
                            let dep_package = ws_members
                                .get(&dep.package_name().to_string())
                                .ok_or(anyhow::format_err!(
                                    "local dependency {} with path {:?} not found in workspace",
                                    dep.package_name(),
                                    dep.source_id().to_string()
                                ))?;
                            if dep_package.name() == self.package.name() {
                                warn!(
                                    "encountered dependency cycle: {:?} <-> {:?}",
                                    self.name(),
                                    package.name()
                                );
                            }
                            if !visited.contains(&dep_package.name()) {
                                queue.push(dep_package);
                            }
                        }
                    }
                }
            }
            Ok(dependencies)
        })
    }

    pub(crate) fn dependencies_filtered<F>(&'a self, f: F) -> Fallible<DependenciesT>
    where
        F: Fn(&&Dependency) -> bool,
        F: Copy,
    {
        let filtered = self
            .dependencies()?
            .into_iter()
            .filter_map(|(name, deps)| {
                let filtered_deps = deps.into_iter().filter(f).collect::<LinkedHashSet<_>>();

                if !filtered_deps.is_empty() {
                    Some((name.clone(), deps.clone()))
                } else {
                    None
                }
            })
            .collect();

        Ok(filtered)
    }

    /// Returns the crates not in the same workspace that this crate depends on.
    pub(crate) fn dependencies_external(&'a self) -> Fallible<DependenciesT> {
        let ws_members = self.workspace.members_unsorted_mapped()?;

        self.dependencies_filtered(|dep| {
            dep.source_id().local_path().is_none()
                || ws_members.get(&dep.package_name().to_string()).is_none()
        })
    }

    /// Returns the crates in the same workspace that this crate depends on.
    pub(crate) fn dependencies_in_workspace(&'a self) -> Fallible<DependenciesT> {
        let ws_members = self.workspace.members_unsorted_mapped()?;

        self.dependencies_filtered(|dep| {
            dep.source_id().local_path().is_some()
                && ws_members.get(&dep.package_name().to_string()).is_some()
        })
    }

    /// Returns a reference to all workspace crates that depend on this crate.
    // todo: write a unit test for this
    pub(crate) fn dependants_in_workspace(&'a self) -> Fallible<&'a Vec<&'a Crate<'a>>> {
        self.dependants_in_workspace_filtered(|_| true)
    }

    /// Returns a reference to all workspace crates that depend on this crate.
    /// Features filtering by applying a filter function to the dependant's dependencies.
    // todo: write a unit test for this
    pub(crate) fn dependants_in_workspace_filtered<F>(
        &'a self,
        filter_fn: F,
    ) -> Fallible<&'a Vec<&'a Crate<'a>>>
    where
        F: Fn(&(&String, &LinkedHashSet<Dependency>)) -> bool,
        F: Copy,
    {
        self.dependants_in_workspace.get_or_try_init(|| {
            let members_dependants = self.workspace.members()?.iter().try_fold(
                LinkedHashMap::<String, &'a Crate<'a>>::new(),
                |mut acc, member| -> Fallible<_> {
                    if member
                        .dependencies_in_workspace()?
                        .iter()
                        .filter(filter_fn)
                        .map(|(name, _)| name)
                        .collect::<HashSet<_>>()
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

    pub(crate) fn root(&self) -> &Path {
        self.package.root()
    }

    pub(crate) fn workspace(&self) -> &'a ReleaseWorkspace<'a> {
        self.workspace
    }
}

type MemberStates = LinkedHashMap<String, CrateState>;

#[derive(custom_debug::Debug)]
pub(crate) struct ReleaseWorkspace<'a> {
    root_path: PathBuf,
    criteria: SelectionCriteria,

    changelog: Option<ChangelogT<'a, WorkspaceChangelog>>,

    #[debug(skip)]
    cargo_config: cargo::util::config::Config,
    cargo_workspace: OnceCell<CargoWorkspace<'a>>,
    members_unsorted: OnceCell<Vec<Crate<'a>>>,
    members_sorted: OnceCell<Vec<&'a Crate<'a>>>,
    members_states: OnceCell<MemberStates>,
    #[debug(skip)]
    git_repo: git2::Repository,
}

/// Configuration criteria for the crate selection.
#[derive(Educe, Debug)]
#[educe(Default)]
pub(crate) struct SelectionCriteria {
    #[educe(Default(expression = r#"fancy_regex::Regex::new(".*").expect("matching anything is valid")"#r))]
    pub(crate) match_filter: fancy_regex::Regex,
    pub(crate) enforced_version_reqs: Vec<semver::VersionReq>,
    pub(crate) disallowed_version_reqs: Vec<semver::VersionReq>,
    pub(crate) allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,
    pub(crate) allowed_selection_blockers: BitFlags<CrateStateFlags>,
    pub(crate) exclude_optional_deps: bool,
}

/// Defines detailed crate's state in terms of the release process.
#[bitflags]
#[repr(u32)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CrateStateFlags {
    /// matches a package filter
    Matched,
    /// in the dependency tree of a matched package
    IsWorkspaceDependency,
    /// in the dev-dependency tree of a matched package, specifying a version requirement
    IsWorkspaceVersionedDevDependency,
    /// in the dev-dependency tree of a matched package, not specifying a version requirement
    IsWorkspaceUnversionedDevDependency,
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
}

/// Defines the meta states that can be derived from the more detailed `CrateStateFlags`.
#[bitflags]
#[repr(u16)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MetaCrateStateFlags {
    Allowed,
    Blocked,
    Changed,
    Selected,
}

impl CrateStateFlags {
    pub(crate) fn empty_set() -> BitFlags<Self> {
        BitFlags::empty()
    }
}

/// Implements the logic for determining a crate's starte in terms of the release process.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CrateState {
    flags: BitFlags<CrateStateFlags>,
    meta_flags: BitFlags<MetaCrateStateFlags>,

    allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,
    allowed_selection_blockers: BitFlags<CrateStateFlags>,
}

impl CrateState {
    pub(crate) const BLOCKING_STATES: BitFlags<CrateStateFlags> = enumflags2::make_bitflags!(
        CrateStateFlags::{MissingChangelog
            | MissingReadme
            | UnreleasableViaChangelogFrontmatter
            | DisallowedVersionReqViolated
            | EnforcedVersionReqViolated
            | MissingDescription
            | MissingLicense
            | HasWildcardDependency
    });

    pub(crate) fn new(
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

    pub(crate) fn contains(&self, flag: CrateStateFlags) -> bool {
        self.flags.contains(flag)
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.flags.extend(other.flags.iter());
        self.update_meta_flags();
    }

    pub(crate) fn insert(&mut self, flag: CrateStateFlags) {
        self.flags.insert(flag);
        self.update_meta_flags();
    }

    pub(crate) fn is_matched(&self) -> bool {
        self.flags.contains(CrateStateFlags::Matched)
    }

    pub(crate) fn is_dependency(&self) -> bool {
        self.flags.contains(CrateStateFlags::IsWorkspaceDependency)
    }

    pub(crate) fn is_versioned_dev_dependency(&self) -> bool {
        self.flags
            .contains(CrateStateFlags::IsWorkspaceVersionedDevDependency)
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

        match (self.is_matched(), self.is_versioned_dev_dependency()) {
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
    pub(crate) fn changed(&self) -> bool {
        self.flags.contains(CrateStateFlags::NoPreviousRelease)
            || self.flags.contains(CrateStateFlags::MissingReleaseTag)
            || self
                .flags
                .contains(CrateStateFlags::ChangedSincePreviousRelease)
    }

    /// At least one dependency is marked as changed.
    pub(crate) fn dependency_changed(&self) -> bool {
        self.flags.contains(CrateStateFlags::DependencyChanged)
    }

    /// There are changes to be released since the previous release
    pub(crate) fn changed_since_previous_release(&self) -> bool {
        self.flags
            .contains(CrateStateFlags::ChangedSincePreviousRelease)
    }

    /// Has a prevoius release.
    pub(crate) fn has_previous_release(&self) -> bool {
        self.flags.contains(CrateStateFlags::HasPreviousRelease)
    }

    /// Has been matched explicitly or as a consequence of a dependency.
    pub(crate) fn selected(&self) -> bool {
        self.is_matched() || self.is_dependency() || self.is_versioned_dev_dependency()
    }

    /// Will be included in the release
    pub(crate) fn release_selection(&self) -> bool {
        !self.blocked() && (self.changed() || self.dependency_changed()) && self.selected()
    }

    /// Returns a formatted string with an overview of crates and their states.
    pub(crate) fn format_crates_states<'cs, CS>(
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

            root_path,
            criteria: Default::default(),
            changelog,
            cargo_config: cargo::util::config::Config::default()?,

            cargo_workspace: Default::default(),
            members_unsorted: Default::default(),
            members_sorted: Default::default(),
            members_states: Default::default(),
        };

        // todo(optimization): eagerly ensure that the workspace is valid, but the following fails lifetime checks
        // let _ = new.cargo_workspace()?;

        Ok(new)
    }

    fn members_states(&'a self) -> Fallible<&MemberStates> {
        self.members_states.get_or_try_init(|| {
            let ws_members = self.members_unsorted_mapped()?;

            let mut members_states = MemberStates::new();

            let criteria = &self.criteria;
            let initial_state = CrateState {
                allowed_dev_dependency_blockers: criteria.allowed_dev_dependency_blockers,
                allowed_selection_blockers: criteria.allowed_selection_blockers,

                ..Default::default()
            };

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

                    // todo: define a const or variable for the joined path
                    if !std::path::Path::new(&member.root().join("README.md")).exists() {
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
                                    if !changed_files(member.package.root(), &git_tag, "HEAD")?
                                        .is_empty()
                                    {
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

                    // dependency state
                    // only dependencies of explicitly matched packages are considered here.
                    //
                    // note(steveej):
                    // while trying to signal the inclusion of reverse dependencies it eventually occurred to me
                    // that only considering the crates in the dependency trees that start with a selected package is preferred.
                    // even if a reverse dependency of a matched package is changed during the release (by having its dependency version updated),
                    // its not relevant to the release if it hasn't been requested for release excplicitly or as a dependency of one that has been, in which case it is already considered.
                    // if get_state!(member.name()).is_matched() && !get_state!(member.name()).blocked()
                    if get_state!(member.name()).is_matched()
                        && get_state!(member.name()).changed()
                        && !get_state!(member.name()).blocked()
                    {
                        for (dep_name, deps) in member.dependencies()? {
                            for dep in deps {
                                assert_eq!(dep_name, &dep.package_name().to_string(), "this is most likely a bug in this program. please submit a bug report.");

                                if dep.specified_req() {
                                    if dep.version_req().to_string().contains('*') {
                                        insert_state!(CrateStateFlags::HasWildcardDependency);
                                    }

                                    match (
                                        ws_members.get(dep_name).is_some(),
                                        dep.kind(),
                                    ) {
                                        (true, CargoDepKind::Development,) =>
                                            insert_state!(CrateStateFlags::IsWorkspaceVersionedDevDependency, dep_name.clone()),
                                        (true, _) => insert_state!(CrateStateFlags::IsWorkspaceDependency, dep_name.clone()),

                                        other => bail!("[{}/{}] unhandled case: {:#?}", member.name(), dep_name, other)
                                    }
                                }
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
    pub(crate) fn release_selection(&'a self) -> Fallible<Vec<&'a Crate>> {
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
            .collect();

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

    /// Returns all non-excluded workspace members.
    /// Members are sorted according to their dependency tree from most independent to most dependent.
    pub(crate) fn members(&'a self) -> Fallible<&'a Vec<&'a Crate<'a>>> {
        self.members_sorted.get_or_try_init(|| -> Fallible<_> {
            let mut members = self.members_unsorted()?.iter().collect::<Vec<_>>();

            let dependencies = self.members_unsorted()?.iter().try_fold(
                HashMap::<String, HashSet<String>>::new(),
                |mut acc, elem| -> Fallible<_> {
                    acc.insert(
                        elem.package.name().to_string(),
                        elem.dependencies_in_workspace()?
                            .into_iter()
                            .map(|(name, _)| name)
                            .collect(),
                    );

                    Ok(acc)
                },
            )?;

            // ensure members are ordered respecting their dependency tree
            members.sort_by(move |a, b| {
                use std::cmp::Ordering::{Equal, Greater, Less};

                let a_deps = dependencies
                    .get(&a.name())
                    .unwrap_or_else(|| panic!("dependencies for {} not found", a.name()));
                let b_deps = dependencies
                    .get(&b.name())
                    .unwrap_or_else(|| panic!("dependencies for {} not found", b.name()));

                let comparison = (a_deps.contains(&b.name()), b_deps.contains(&a.name()));
                let result = match comparison {
                    (true, true) => {
                        panic!("cyclic dependency between {} and {}", a.name(), b.name())
                    }
                    (true, false) => Greater,
                    (false, true) => Less,
                    (false, false) => Equal,
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

            Ok(members)
        })
    }

    /// Return the root path of the workspace.
    pub(crate) fn root(&'a self) -> &Path {
        &self.root_path
    }

    pub(crate) fn git_repo(&'a self) -> &git2::Repository {
        &self.git_repo
    }

    /// Tries to resolve the git HEAD to its corresponding branch.
    pub(crate) fn git_head_branch(&'a self) -> Fallible<(git2::Branch, git2::BranchType)> {
        for branch in self.git_repo.branches(None)? {
            let branch = branch?;
            if branch.0.is_head() {
                return Ok(branch);
            }
        }

        bail!("head branch not found")
    }

    /// Calls Self::git_head_branch and tries to resolve its name to String.
    pub(crate) fn git_head_branch_name(&'a self) -> Fallible<String> {
        self.git_head_branch().map(|(branch, _)| {
            branch
                .name()?
                .map(String::from)
                .ok_or_else(|| anyhow::anyhow!("the current git branch has no name"))
        })?
    }

    /// Creates a git branch with the given name off of the current HEAD, optionally overwriting the branch if it exists.
    pub(crate) fn git_checkout_branch(&'a self, name: &str, force: bool) -> Fallible<git2::Branch> {
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
    pub(crate) fn git_checkout_new_branch(&'a self, name: &str) -> Fallible<git2::Branch> {
        self.git_checkout_branch(name, false)
    }

    // todo: make this configurable?
    fn git_signature(&self) -> Fallible<git2::Signature> {
        Ok(git2::Signature::now(
            "Holochain Core Dev Team",
            "devcore@holochain.org",
        )?)
    }

    /// Add the given files and create a commit.
    pub(crate) fn git_add_all_and_commit(
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
    pub(crate) fn git_tag(&self, name: &str, force: bool) -> Fallible<git2::Oid> {
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

    pub(crate) fn changelog(&'a self) -> Option<&'a ChangelogT<'a, WorkspaceChangelog>> {
        self.changelog.as_ref()
    }

    pub(crate) fn update_lockfile<T>(
        &'a self,
        dry_run: bool,
        additional_manifests: T,
    ) -> Fallible<()>
    where
        T: Iterator<Item = &'a str>,
    {
        for args in [
            vec![
                vec!["fetch", "--verbose"],
                [
                    vec!["update", "--workspace", "--offline", "--verbose"],
                    if dry_run { vec!["--dry-run"] } else { vec![] },
                ]
                .concat(),
            ],
            if dry_run {
                vec![]
            } else {
                additional_manifests
                    .map(|mp| vec!["generate-lockfile", "--offline", "--manifest-path", mp])
                    .collect::<Vec<_>>()
            },
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

    pub(crate) fn cargo_check(&'a self, offline: bool) -> Fallible<()> {
        let mut cmd = std::process::Command::new("cargo")
            .current_dir(self.root())
            .args(
                &[
                    vec![
                        "check",
                        "--workspace",
                        "--all-targets",
                        "--all-features",
                        "--release",
                    ],
                    if offline { vec!["--offline"] } else { vec![] },
                ]
                .concat(),
            )
            .spawn()?;

        let cmd_status = cmd.wait()?;

        if !cmd_status.success() {
            bail!("running {:?} failed: \n{:?}", cmd, cmd.stderr);
        }

        Ok(())
    }

    fn members_unsorted_mapped(&'a self) -> Fallible<HashMap<String, CargoPackage>> {
        Ok(self
            .members_unsorted()?
            .iter()
            .map(|m| (m.name(), m.package.clone()))
            .collect())
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
pub(crate) fn git_lookup_tag(git_repo: &git2::Repository, tag_name: &str) -> Option<String> {
    let tag = git_repo
        .revparse_single(tag_name)
        .ok()
        .map(|obj| obj.as_tag().cloned())
        .flatten()
        .map(|tag| tag.name().unwrap_or_default().to_owned());

    trace!("looking up tag '{}' -> {:?}", tag_name, tag);

    tag
}

#[cfg(test)]
pub(crate) mod tests;
