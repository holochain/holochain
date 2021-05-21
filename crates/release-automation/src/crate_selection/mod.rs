//! Select which crates to include in the release process.

use crate::changelog::{self, ChangeType, CrateChangelog};
use crate::Fallible;
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

#[derive(custom_debug::Debug)]
pub(crate) struct Crate<'a> {
    package: CargoPackage,
    changelog: Option<CrateChangelog<'a>>,
    #[debug(with = "releaseworkspace_path_only_fmt")]
    workspace: &'a ReleaseWorkspace<'a>,
    #[debug(skip)]
    dependencies_in_workspace: OnceCell<LinkedHashSet<cargo::core::Dependency>>,
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
                Some(crate::changelog::CrateChangelog::try_from_path(
                    &changelog_path,
                )?)
            } else {
                None
            }
        };

        Ok(Self {
            package,
            changelog,
            workspace,
            dependencies_in_workspace: Default::default(),
        })
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

    /// This crate's changelog.
    pub(crate) fn changelog(&'a self) -> Option<&CrateChangelog<'a>> {
        self.changelog.as_ref()
    }

    /// Returns the crates in the same workspace that this crate depends on.
    pub(crate) fn dependencies_in_workspace(
        &'a self,
    ) -> Fallible<&'a LinkedHashSet<cargo::core::Dependency>> {
        self.dependencies_in_workspace.get_or_try_init(|| {
            // LinkedHashSet automatically deduplicates while maintaining the insertion order.
            let mut dependencies = LinkedHashSet::new();
            let ws_members: std::collections::HashMap<_, _> = self
                .workspace
                .members_unsorted()?
                .iter()
                .map(|m| (m.name(), &m.package))
                .collect();

            // This vector is used to implement a depth-first-search to capture all transitive dependencies.
            // Starting with the package in self and traversing down from it.
            let mut queue = vec![&self.package];

            while let Some(package) = queue.pop() {
                for dep in package.dependencies() {
                    if dep.source_id().is_path() {
                        let dep_name = dep.package_name().to_string();
                        let dep_kind = dep.kind();

                        // todo: write a test-case for this
                        if dep.is_optional() && self.workspace.criteria.exclude_optional_deps {
                            debug!(
                                "[{}] excluding optional dependency '{}'",
                                package.name(),
                                dep_name,
                            );

                            continue;
                        }

                        // todo: write a test-case for this
                        if self
                            .workspace
                            .criteria
                            .exclude_dep_kinds
                            .contains(&dep_kind)
                        {
                            debug!(
                                "[{}] excluding {:?} dependency '{}'",
                                package.name(),
                                dep_kind,
                                dep_name,
                            );

                            continue;
                        }

                        // todo(backlog): could the path of this dependency possibly be outside of the workspace?
                        dependencies.insert(dep.to_owned());

                        // todo: potentially remove? as it's not our job to detect this
                        if let Some(dep_package) = ws_members.get(&dep.package_name().to_string()) {
                            if dep_package.name() == self.package.name() {
                                bail!(
                                    "encountered dependency cycle: {:?} <-> {:?}",
                                    self.name(),
                                    package.name()
                                );
                            }

                            queue.push(dep_package);
                        }
                    }
                }
            }
            Ok(dependencies)
        })
    }

    pub(crate) fn root(&self) -> &Path {
        self.package.root()
    }
}

type MemberStates = LinkedHashMap<String, CrateState>;

#[derive(custom_debug::Debug)]
pub(crate) struct ReleaseWorkspace<'a> {
    // initialised: bool,
    root_path: PathBuf,
    criteria: SelectionCriteria,

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
    pub(crate) selection_filter: fancy_regex::Regex,
    pub(crate) enforced_version_reqs: Vec<semver::VersionReq>,
    pub(crate) disallowed_version_reqs: Vec<semver::VersionReq>,
    pub(crate) allowed_dependency_blockers: BitFlags<CrateStateFlags>,
    pub(crate) allowed_selection_blockers: BitFlags<CrateStateFlags>,
    pub(crate) exclude_dep_kinds: HashSet<CargoDepKind>,
    pub(crate) exclude_optional_deps: bool,
}

/// Defines detailed crate's state in terms of the release process.
#[bitflags]
#[repr(u16)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CrateStateFlags {
    /// matches a package filter
    Matched,
    /// in the dependency tree of a matched package
    IsWorkspaceDependency,
    /// has changed since previous release if any
    HasPreviousRelease,
    /// has changed since previous release
    ChangedSincePreviousRelease,

    /// has `unreleasable: true` set in changelog
    MissingChangelog,
    MissingReadme,
    UnreleasableViaChangelogFrontmatter,
    EnforcedVersionReqViolated,
    DisallowedVersionReqViolated,
}

/// Defines the meta states that can be derived from the more detailed `CrateStateFlags`.
#[bitflags]
#[repr(u16)]
#[derive(enum_utils::FromStr, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MetaCrateStateFlags {
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

    allowed_dependency_blockers: BitFlags<CrateStateFlags>,
    allowed_selection_blockers: BitFlags<CrateStateFlags>,
}

impl CrateState {
    pub(crate) const BLOCKING_STATES: BitFlags<CrateStateFlags> = enumflags2::make_bitflags!(
        CrateStateFlags::{MissingChangelog |
            MissingReadme|
            UnreleasableViaChangelogFrontmatter |
            DisallowedVersionReqViolated|
            EnforcedVersionReqViolated
    });

    pub(crate) fn new(
        flags: BitFlags<CrateStateFlags>,
        allowed_dependency_blockers: BitFlags<CrateStateFlags>,
        allowed_selection_blockers: BitFlags<CrateStateFlags>,
    ) -> Self {
        let mut new = Self {
            flags,
            meta_flags: Default::default(),
            allowed_dependency_blockers,
            allowed_selection_blockers,
        };
        new.update_meta_flags();
        new
    }

    pub(crate) fn merge(&mut self, other: Self) -> () {
        self.flags.extend(other.flags.iter());
        self.update_meta_flags();
    }

    pub(crate) fn insert(&mut self, flag: CrateStateFlags) -> () {
        self.flags.insert(flag);
        self.update_meta_flags();
    }

    pub(crate) fn is_matched(&self) -> bool {
        self.flags.contains(CrateStateFlags::Matched)
    }

    pub(crate) fn is_dependency(&self) -> bool {
        self.flags.contains(CrateStateFlags::IsWorkspaceDependency)
    }

    fn update_meta_flags(&mut self) -> () {
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

        if self.selected() {
            self.meta_flags.insert(MetaCrateStateFlags::Selected);
        } else {
            self.meta_flags.remove(MetaCrateStateFlags::Selected);
        }
    }

    fn blocked_by(&self) -> BitFlags<CrateStateFlags> {
        let mut blocking_flags = Self::BLOCKING_STATES.clone();

        match (self.is_matched(), self.is_dependency()) {
            (true, _) => blocking_flags.remove(self.allowed_selection_blockers),
            (false, true) => blocking_flags.remove(self.allowed_dependency_blockers),
            (false, false) => {}
        }

        blocking_flags.intersection_c(self.flags)
    }

    fn blocked(&self) -> bool {
        !self.blocked_by().is_empty()
    }

    /// There are changes to be released.
    pub(crate) fn changed(&self) -> bool {
        !self.flags.contains(CrateStateFlags::HasPreviousRelease)
            || self
                .flags
                .contains(CrateStateFlags::ChangedSincePreviousRelease)
    }

    /// Has been selected either explicitly or as a dependency.
    pub(crate) fn selected(&self) -> bool {
        self.is_matched() || self.is_dependency()
    }

    /// Will be included in the release
    pub(crate) fn release_selection(&self) -> bool {
        !self.blocked() && (self.changed() || self.selected())
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
            states_shown += "Blocking "
        }
        if show_flags {
            states_shown += "Flags "
        }
        if show_meta {
            states_shown += "Meta"
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
                    blocking_flags = state.blocked_by().iter().collect::<Vec<_>>(),
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

            msg += &format!("\n");
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

    pub fn try_new(root_path: PathBuf) -> Fallible<ReleaseWorkspace<'a>> {
        let new = Self {
            // initialised: false,
            git_repo: git2::Repository::open(&root_path)?,

            root_path,
            criteria: Default::default(),
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
            let mut members_states = MemberStates::new();

            let criteria = &self.criteria;
            let initial_state = CrateState {
                allowed_dependency_blockers: criteria.allowed_dependency_blockers,
                allowed_selection_blockers: criteria.allowed_selection_blockers,

                ..Default::default()
            };

            for member in self.members()? {
                // helper macros to access the desired state
                macro_rules! get_state {
                    ( $i:expr ) => {
                        members_states.entry($i).or_insert(initial_state.clone())
                    };
                };
                macro_rules! insert_state {
                    ( $flag:expr ) => {
                        insert_state!($flag, member.name())
                    };
                    ( $flag:expr, $i:expr ) => {
                        get_state!($i).insert($flag)
                    };
                };

                // regex matching state
                if criteria.selection_filter.is_match(&member.name())? {
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

                    // dependency state
                    if get_state!(member.name()).is_matched() {
                        for dep in member.dependencies_in_workspace()? {
                            insert_state!(
                                CrateStateFlags::IsWorkspaceDependency,
                                dep.package_name().to_string()
                            );
                        }
                    }

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

                            if let Some(previous_release) = changelog
                                .changes()
                                .ok()
                                // .flatten()
                                .iter()
                                .flatten()
                                .filter_map(|r| {
                                    if !r.change_type().is_unreleased() {
                                        Some(r.title.clone())
                                    } else {
                                        None
                                    }
                                })
                                .take(1)
                                .next()
                            {
                                // lookup the git tag for the previous release
                                let git_tag = self
                                    .git_repo
                                    // todo: derive the tagname from a function
                                    .revparse_single(&format!(
                                        "{}-v{}",
                                        member.name(),
                                        previous_release
                                    ))
                                    .ok()
                                    .map(|obj| obj.id())
                                    .map(|id| self.git_repo.find_tag(id).ok())
                                    .flatten()
                                    .map(|tag| tag.name().unwrap_or_default().to_owned());

                                if let Some(git_tag) = &git_tag {
                                    insert_state!(CrateStateFlags::HasPreviousRelease);

                                    // todo: make comparison ref configurable
                                    if !changed_files(member.package.root(), git_tag, "HEAD")?
                                        .is_empty()
                                    {
                                        insert_state!(CrateStateFlags::ChangedSincePreviousRelease)
                                    }
                                }
                            }
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
    pub(crate) fn release_selection<'b>(&'a self) -> Fallible<Vec<&'a Crate>> {
        let members = self.members()?;

        let all_crates_states_iter = members.iter().map(|member| (member.name(), member.state()));
        let all_crates_states = all_crates_states_iter.clone().collect::<Vec<_>>();
        trace!(
            "{}",
            CrateState::format_crates_states(&all_crates_states, "ALL CRATES", true, true, true,)
        );
        let blocked_crates_states = all_crates_states_iter
            .clone()
            .filter(|(_, state)| state.selected() && state.blocked())
            .collect::<Vec<_>>();

        // indicate an error if any unreleasable crates block the release
        if blocked_crates_states.is_empty() == false {
            bail!(
                "the following crates are blocked but required for the release: \n{}",
                CrateState::format_crates_states(
                    &blocked_crates_states,
                    "BLOCKED CRATES",
                    true,
                    false,
                    false,
                )
            )
        }

        let release_selection = Vec::from_iter(
            members
                .into_iter()
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
                .cloned(),
        );

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
            let mut members = Vec::from_iter(self.members_unsorted()?.into_iter());

            let dependencies = self.members_unsorted()?.into_iter().try_fold(
                HashMap::<String, HashSet<String>>::new(),
                |mut acc, elem| -> Fallible<_> {
                    acc.insert(
                        elem.package.name().to_string().to_owned(),
                        elem.dependencies_in_workspace()?
                            .iter()
                            .map(|dep| dep.package_name().to_string().clone())
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
                    .expect(&format!("dependencies for {} not found", a.name()));
                let b_deps = dependencies
                    .get(&b.name())
                    .expect(&format!("dependencies for {} not found", b.name()));

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

    pub(crate) fn root(&'a self) -> Fallible<&Path> {
        Ok(self.cargo_workspace()?.root())
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

#[cfg(test)]
pub(crate) mod tests;
