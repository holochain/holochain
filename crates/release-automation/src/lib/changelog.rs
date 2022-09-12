use crate::common::SemverIncrementMode;
use crate::crate_selection::Crate;
use crate::release::ReleaseWorkspace;
use crate::Fallible;
use anyhow::bail;
use comrak::nodes::Ast;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{format_commonmark, parse_document, Arena, ComrakOptions};
use log::{debug, trace, warn};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::io::Write;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::{cell::RefCell, convert::TryFrom};
use std::{collections::HashSet, convert::TryInto};

#[derive(Clone, Default, Debug, PartialEq, Deserialize, Serialize)]
pub(crate) struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    unreleasable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_unreleasable: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    semver_increment_mode: Option<SemverIncrementMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_semver_increment_mode: Option<SemverIncrementMode>,
}

impl Frontmatter {
    pub(crate) fn unreleasable(&self) -> bool {
        self.unreleasable
            .unwrap_or_else(|| self.default_unreleasable.unwrap_or_default())
    }

    pub(crate) fn semver_increment_mode(&self) -> SemverIncrementMode {
        self.semver_increment_mode.clone().unwrap_or_else(|| {
            self.default_semver_increment_mode
                .clone()
                .unwrap_or_default()
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.unreleasable.is_none()
            && self.default_unreleasable.is_none()
            && self.semver_increment_mode.is_none()
            && self.default_semver_increment_mode.is_none()
    }

    /// Remove any non-default values in the frontmatter.
    pub(crate) fn reset_to_defaults(&mut self) {
        if self.unreleasable.is_some() {
            self.unreleasable = None;
        }

        if self.semver_increment_mode.is_some() {
            self.semver_increment_mode = None;
        }
    }
}

/// Trims potential brackets and spaces
pub(crate) fn normalize_heading_name(input: &str) -> String {
    input.replace("[", "").replace("]", "").replace(" ", "")
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReleaseChange {
    CrateReleaseChange(String),
    WorkspaceReleaseChange(String, Vec<String>),
}

impl ReleaseChange {
    pub(crate) fn title(&self) -> &str {
        match self {
            ReleaseChange::CrateReleaseChange(t) => t,
            ReleaseChange::WorkspaceReleaseChange(t, _) => t,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChangeT {
    Release(ReleaseChange),
    Unreleased,
    Changelog,
    None,
}

impl<'a> ChangeT {
    pub(crate) fn from_heading_node(
        node: &'a comrak::arena_tree::Node<'a, RefCell<Ast>>,
        level: u32,
    ) -> Fallible<Self> {
        let mut change = Self::None;

        for sibling in node.following_siblings() {
            if let NodeValue::Heading(heading) = sibling.data.borrow().value {
                if heading.level == level {
                    if change != ChangeT::None {
                        break;
                    }

                    let title = get_heading_text(sibling)
                        .ok_or_else(|| anyhow::anyhow!("no heading text found"))?;

                    let trimmed = normalize_heading_name(&title).to_lowercase();

                    match trimmed.as_str() {
                        "unreleased" => {
                            change = Self::Unreleased;
                            break;
                        }
                        "changelog" => {
                            change = Self::Changelog;
                            break;
                        }
                        _ => {}
                    }

                    match level {
                        WorkspaceChangelog::RELEASE_HEADING_LEVEL => {
                            change = ChangeT::Release(ReleaseChange::WorkspaceReleaseChange(
                                title,
                                vec![],
                            ));
                        }
                        CrateChangelog::RELEASE_HEADING_LEVEL => {
                            change = ChangeT::Release(ReleaseChange::CrateReleaseChange(title));
                        }

                        _ => {}
                    }
                } else if heading.level == level + 1
                    && level == WorkspaceChangelog::RELEASE_HEADING_LEVEL
                {
                    if let ChangeT::Release(ReleaseChange::WorkspaceReleaseChange(
                        _,
                        ref mut crate_releases,
                    )) = change
                    {
                        let crate_release_title = get_heading_text(sibling)
                            .ok_or_else(|| anyhow::anyhow!("no heading text found"))?;

                        crate_releases.push(crate_release_title);
                    };
                }
            }
        }

        Ok(change)
    }
}

impl From<ChangeT> for Option<ReleaseChange> {
    fn from(c: ChangeT) -> Self {
        match c {
            ChangeT::Release(rc) => Some(rc),
            _ => None,
        }
    }
}

impl ChangeT {
    pub(crate) fn title(&self) -> Option<String> {
        match self {
            Self::Release(rc) => Some(rc.title().to_string()),
            _ => None,
        }
    }
}

#[derive(custom_debug::Debug)]
pub(crate) struct Machinery<'a> {
    path: PathBuf,
    #[debug(skip)]
    arena: Arena<AstNode<'a>>,
    #[debug(skip)]
    root: OnceCell<&'a comrak::arena_tree::Node<'a, RefCell<Ast>>>,
    #[debug(skip)]
    options: ComrakOptions,
}

impl Machinery<'_> {
    pub(crate) fn with_path(path: &Path) -> Self {
        Self {
            path: path.to_owned(),

            ..Default::default()
        }
    }
}

impl<'a> Default for Machinery<'a> {
    fn default() -> Self {
        let path = Default::default();
        let arena = Arena::new();
        let root = Default::default();

        let mut options = ComrakOptions::default();
        options.parse.smart = true;
        options.extension.front_matter_delimiter = Some("---".to_owned());
        options.render.hardbreaks = true;

        Self {
            path,
            arena,
            root,
            options,
        }
    }
}

/// Workaround until Rust supports passing enum variants as types.
#[derive(Debug)]
pub(crate) enum ChangelogType {
    Crate,
    Workspace,
}

#[derive(Debug)]
pub(crate) enum Changelog<'a> {
    Crate(Machinery<'a>),
    Workspace(Machinery<'a>),
}

pub(crate) const WORKSPACE_RELEASE_HEADING_LEVEL: u32 = 1;
pub(crate) const CRATE_RELEASE_HEADING_LEVEL: u32 = 2;

use core::marker::PhantomData;

#[derive(Debug)]
pub(crate) struct CrateChangelog;
#[derive(Debug)]
pub(crate) struct WorkspaceChangelog;
#[derive(Debug)]
pub(crate) struct ChangelogT<'a, T>(Machinery<'a>, PhantomData<T>);
pub(crate) trait HeadingLevel {
    const RELEASE_HEADING_LEVEL: u32;
}

impl HeadingLevel for CrateChangelog {
    const RELEASE_HEADING_LEVEL: u32 = CRATE_RELEASE_HEADING_LEVEL;
}

impl HeadingLevel for WorkspaceChangelog {
    const RELEASE_HEADING_LEVEL: u32 = WORKSPACE_RELEASE_HEADING_LEVEL;
}

impl<'a, T> ChangelogT<'a, T>
where
    T: HeadingLevel,
{
    pub(crate) fn at_path(path: &Path) -> Self {
        Self(Machinery::with_path(path), PhantomData::<T>)
    }

    fn root(&'a self) -> Fallible<&&'a comrak::arena_tree::Node<'a, RefCell<Ast>>> {
        self.0.root.get_or_try_init(|| {
            let s = std::fs::read_to_string(&self.0.path)?;
            Ok(parse_document(&self.0.arena, &s, &self.0.options))
        })
    }

    pub(crate) fn path(&'a self) -> &'a Path {
        &self.0.path
    }

    fn arena(&'a self) -> &Arena<AstNode<'a>> {
        &self.0.arena
    }

    fn options(&'a self) -> &ComrakOptions {
        &self.0.options
    }

    pub(crate) fn changes(&'a self) -> Fallible<Vec<ChangeT>> {
        let root = self.root()?;
        let mut changes = vec![];

        for (i, node) in root.children().enumerate() {
            // we're only interested in the headings here
            if let NodeValue::Heading(heading) = node.data.borrow().value {
                let mut msg = format!("[{}] heading at level {}", i, heading.level);

                if heading.level == T::RELEASE_HEADING_LEVEL {
                    match ChangeT::from_heading_node(node, T::RELEASE_HEADING_LEVEL)? {
                        ChangeT::None => {}
                        change => {
                            msg += &format!(" => [{}] derived change '{:?}'", i, change);
                            changes.push(change);
                        }
                    }
                }

                trace!("{}", msg);
            }
        }

        Ok(changes)
    }

    fn changes_filtered<F>(&'a self, filter: F) -> Fallible<Vec<ChangeT>>
    where
        F: FnMut(&ChangeT) -> bool,
    {
        Ok(self.changes()?.into_iter().filter(filter).collect())
    }

    pub(crate) fn topmost_release(&'a self) -> Fallible<Option<ReleaseChange>> {
        Ok(self
            .changes_filtered(|change| matches!(change, ChangeT::Release(_)))?
            .into_iter()
            .map(Into::into)
            .take(1)
            .next()
            .flatten())
    }

    /// Find and parse the frontmatter of this crate's changelog file.
    pub(crate) fn front_matter(&'a self) -> Fallible<Option<Frontmatter>> {
        for (i, node) in self.root()?.children().enumerate() {
            {
                let children = node.children().count();
                let descendants = node.descendants().count();
                let debug = format!("{:#?}", node.data.borrow().value);
                let ty = debug
                    .split(&['(', ' '][..])
                    .next()
                    .ok_or_else(|| format!("error extracting type from '{}'", debug))
                    .map_err(anyhow::Error::msg)?;
                trace!(
                    "[{}] {} with {} child(ren) and {} descendant(s)",
                    i,
                    ty,
                    children,
                    descendants
                );
            }

            // we're only interested in the frontmatter here
            if let NodeValue::FrontMatter(ref fm) = &mut node.data.borrow_mut().value {
                let fm_str = String::from_utf8(fm.to_vec())?
                    .replace("---", "")
                    .trim()
                    .to_owned();

                let fm: Frontmatter = if fm_str.is_empty() {
                    Frontmatter::default()
                } else {
                    serde_yaml::from_str(&fm_str)?
                };

                trace!(
                    "[{}] found a YAML front matter: {:#?}\nsource string: \n'{}'",
                    i,
                    fm,
                    fm_str
                );

                return Ok(Some(fm));
            }
        }

        Ok(None)
    }
}

impl<'a> HeadingLevel for ChangelogT<'a, CrateChangelog> {
    const RELEASE_HEADING_LEVEL: u32 = CrateChangelog::RELEASE_HEADING_LEVEL;
}

impl<'a> ChangelogT<'a, CrateChangelog> {
    /// Create a new release heading for the items currently under the Unreleased heading.
    /// The target heading will be created regardless of whether one with the same name exists.
    pub(crate) fn add_release(&'a self, title: String) -> Fallible<()> {
        let root = self.root()?;

        let mut unreleased_node = None;
        let mut topmost_release = None;

        for (i, node) in root.children().enumerate() {
            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                let mut msg = format!("[{}] heading at level {}", i, heading.level);

                if heading.level == CrateChangelog::RELEASE_HEADING_LEVEL {
                    if let Some(text_str) = get_heading_text(node) {
                        msg += &format!(" => [{}] found heading text '{}'", i, text_str);

                        if text_str.to_lowercase().contains("unreleased") {
                            // identified unreleased heading

                            if let Some(top) = topmost_release {
                                bail!(
                                    "expected the unreleased heading to be first heading with level {}. found instead: {:?}",
                                    Self::RELEASE_HEADING_LEVEL,
                                    get_heading_text(top)
                                );
                            }

                            msg += " => found unreleased section";
                            unreleased_node = Some(node);
                            break;
                        };
                    }

                    if topmost_release.is_none() {
                        topmost_release = Some(node);
                    }
                }

                trace!("{}", msg);
            }
        }

        // construct the new heading node
        let heading_value = NodeValue::Heading(comrak::nodes::NodeHeading {
            level: Self::RELEASE_HEADING_LEVEL,
            setext: false,
        });
        let heading_ast = comrak::nodes::Ast::new(heading_value);
        let heading = self
            .arena()
            .alloc(comrak::arena_tree::Node::new(core::cell::RefCell::new(
                heading_ast,
            )));

        let text_value = NodeValue::Text(title.into_bytes());
        let text_ast = comrak::nodes::Ast::new(text_value);
        let text = self
            .arena()
            .alloc(comrak::arena_tree::Node::new(core::cell::RefCell::new(
                text_ast,
            )));
        heading.append(text);

        match (topmost_release, unreleased_node) {
            (Some(_), Some(_)) => {
                unreachable!("the loop should have bailed on this condition")
            }

            // no release nor unreleased headings found, append at the end of the document.
            (None, None) => root.append(heading),

            // no unreleased heading found, insert before the first release heading.
            (Some(top), None) => top.insert_before(heading),

            // unreleased heading found, insert immediately after it, thus
            // passing on its content to the new release heading.
            (None, Some(unreleased)) => unreleased.insert_after(heading),
        };

        // write the file
        let mut buf = vec![];
        format_commonmark(root, self.options(), &mut buf).unwrap();
        let mut output_file = std::fs::File::create(&self.path())?;
        output_file.write_all(&buf)?;
        output_file.flush()?;

        Ok(())
    }

    fn erase_front_matter(&'a self, write_file: bool) -> Fallible<String> {
        let frontmatter_re = regex::Regex::new(r"(?ms)^---$.*^---$\w*").unwrap();
        let cl = sanitize(std::fs::read_to_string(self.path())?);

        let cl_edited = sanitize(frontmatter_re.replace(&cl, "").to_string());

        if write_file {
            std::fs::File::create(&self.path())?.write_all(cl_edited.as_bytes())?;
        }

        trace!("changelog without fm:\n{}", cl_edited);

        Ok(cl_edited)
    }

    /// Writes the given Frontmatter back to the changelog file
    fn set_front_matter(&'a self, fm: &Frontmatter) -> Fallible<()> {
        let cl_str = if self.front_matter()?.is_some() {
            self.erase_front_matter(false)?
        } else {
            std::fs::read_to_string(self.path())?
        };

        let cl_final = sanitize(if fm.is_empty() {
            cl_str
        } else {
            let fm_str = serde_yaml::to_string(&fm)?;
            trace!("new frontmatter:\n{}", fm_str);
            indoc::formatdoc!("---\n{}---\n\n{}", fm_str, cl_str)
        });

        trace!("new changelog:\n{}", cl_final);

        std::fs::File::create(&self.path())?.write_all(cl_final.as_bytes())?;

        Ok(())
    }

    /// Calls `Frontmatter::reset_to_defaults`
    pub(crate) fn reset_front_matter_to_defaults(&'a self) -> Fallible<()> {
        if let Some(fm) = self.front_matter()? {
            let mut fm_reset = fm.clone();
            fm_reset.reset_to_defaults();

            if fm != fm_reset {
                return self.set_front_matter(&fm_reset);
            }
        }

        Ok(())
    }
}

impl<'a> HeadingLevel for ChangelogT<'a, WorkspaceChangelog> {
    const RELEASE_HEADING_LEVEL: u32 = WorkspaceChangelog::RELEASE_HEADING_LEVEL;
}

impl<'a> ChangelogT<'a, WorkspaceChangelog> {
    pub(crate) fn aggregate(&'a self, inputs: &[&'a Crate<'a>]) -> Fallible<()> {
        let root = self.root()?;
        let arena = self.arena();

        let mut unreleased_node = None;
        let mut remove_other = false;
        let mut topmost_release = None;

        for (i, node) in root.children().enumerate() {
            match &node.data.borrow().value {
                &NodeValue::Heading(heading) => {
                    let mut msg = format!(
                        "[{:?}/{}] heading at level {}",
                        self.path(),
                        i,
                        heading.level
                    );

                    match (unreleased_node, heading.level) {
                        (Some(_), 1) => {
                            msg += " => arrived at next release section, stopping.";
                            topmost_release = Some(node);
                            break;
                        }
                        (Some(_), _) => {
                            // todo: only remove entries for crates that are given as inputs?
                            msg += " => detaching";
                            remove_other = true;
                            node.detach();
                        }
                        (None, 1) => {
                            if let Some(text_str) = get_heading_text(node) {
                                msg += &format!(" => [{}] found heading text '{}'", i, text_str);

                                if text_str.to_lowercase().contains("unreleased") {
                                    msg += " => found unreleased section";
                                    unreleased_node = Some(node);
                                    remove_other = false;
                                };
                            }
                        }
                        (None, _) => {}
                    };

                    trace!("{}", msg);
                }

                other => {
                    let mut msg = format!("[{}] ", i);
                    if remove_other {
                        msg += "detaching ";
                        node.detach();
                    } else {
                        msg += "keeping ";
                    }

                    match other {
                        NodeValue::Text(ref text) => {
                            msg += &format!("'{}'", String::from_utf8_lossy(text))
                        }
                        _ => msg += &format!("{:?}", other),
                    }

                    trace!("{}", msg);
                }
            };
        }

        if unreleased_node.is_none() {
            todo!("insert unrelased node?")
        }

        // insert the unreleased content into the output file
        for (name, crate_changelog) in inputs.iter().map(|crt| (crt.name(), crt.changelog())) {
            let crate_root = if let Some(cl) = crate_changelog {
                cl.root()?
            } else {
                debug!("crate {} has no changelog", name);
                continue;
            };

            let mut content_unreleased_heading = None;

            for (i, node) in crate_root.children().enumerate() {
                {
                    let children = node.children().count();
                    let descendants = node.descendants().count();
                    let debug = format!("{:#?}", node.data.borrow().value);
                    let ty = debug.split(&['(', ' '][..]).next().unwrap();
                    trace!(
                        "[{}/{}] {} with {} child(ren) and {} descendant(s)",
                        name,
                        i,
                        ty,
                        children,
                        descendants
                    );
                }

                if let NodeValue::Heading(heading) = &mut node.data.borrow_mut().value {
                    trace!(
                        "[{}/{}] found heading with level {}",
                        name,
                        i,
                        heading.level
                    );

                    // look for the 'unreleased' heading
                    if heading.level == CrateChangelog::RELEASE_HEADING_LEVEL
                        && content_unreleased_heading.is_none()
                    {
                        // `descendants()` starts with the node itself so we skip it
                        let search = node.descendants().skip(1).collect::<Vec<_>>();

                        trace!("[{}/{}] searching through {} nodes", name, i, search.len());

                        let mut recent_link_index = None;

                        'two: for (j, node_j) in search
                            .iter()
                            .take_while(|child| child.data.try_borrow().is_ok())
                            .enumerate()
                        {
                            match &mut node_j.data.borrow_mut().value {
                                NodeValue::Link(ref mut _link) => {
                                    trace!("[{}/{}/{}] found link", name, i, j);
                                    recent_link_index = Some(j);
                                }

                                NodeValue::Text(ref mut text) => {
                                    let text_str = String::from_utf8_lossy(text);
                                    if text_str.to_lowercase().contains("unreleased") {
                                        trace!(
                                            "[{}/{}/{}] found unreleased heading: {:#?}",
                                            name,
                                            i,
                                            j,
                                            text_str
                                        );
                                        content_unreleased_heading = Some(node);

                                        *text = name.to_string().as_bytes().to_vec();

                                        trace!("[{}/{}/{}] changing name to {}", name, i, j, name);

                                        let url =
                                                // todo: derive this path dynamically
                                                format!("crates/{}/CHANGELOG.md#unreleased", name);

                                        if let Some(link_index) = recent_link_index {
                                            if let NodeValue::Link(ref mut link) =
                                                search[link_index].data.borrow_mut().value
                                            {
                                                link.url = url.as_bytes().to_vec();
                                                trace!(
                                                    "[{}/{}/{}] changing link to: {:#?}",
                                                    name,
                                                    i,
                                                    j,
                                                    url
                                                );
                                            }
                                        } else {
                                            let link_value =
                                                NodeValue::Link(comrak::nodes::NodeLink {
                                                    url: url.as_bytes().to_vec(),
                                                    title: Default::default(),
                                                });
                                            let ast = comrak::nodes::Ast::new(link_value);
                                            let link = arena.alloc(comrak::arena_tree::Node::new(
                                                core::cell::RefCell::new(ast),
                                            ));
                                            // insert the link node before the text node
                                            node_j.insert_before(link);

                                            // attach the text node as a child of the link
                                            node_j.detach();
                                            link.append(node_j);
                                        }

                                        break 'two;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                };
            }

            let target = match (unreleased_node, topmost_release) {
                (_, Some(topmost)) => topmost,
                (Some(unreleased), _) => unreleased,
                _ => panic!("expected at least one set"),
            };

            // add all siblings between here and the next headline
            let count = content_unreleased_heading
                .map(|content_unreleased_heading| {
                    let mut new_nodes = vec![content_unreleased_heading];

                    new_nodes.extend(
                        content_unreleased_heading
                            .following_siblings()
                            .skip(1)
                            .take_while(|node| {
                                node.descendants().all(|descendant| {
                                    match descendant.data.borrow().value {
                                        NodeValue::Heading(heading) => {
                                            let heading_text = get_nested_text(descendant);
                                            let add = heading.level
                                                > CrateChangelog::RELEASE_HEADING_LEVEL;
                                            trace!(
                                                "adding heading with text '{}' and level {}? {}",
                                                heading_text,
                                                heading.level,
                                                add
                                            );
                                            add
                                        }

                                        _ => true,
                                    }
                                })
                            }),
                    );
                    if new_nodes.len() == 1 {
                        trace!("[{}] skipping empty unreleased heading", name);

                        0
                    } else {
                        for new_node in &new_nodes {
                            let text = get_nested_text(new_node);
                            trace!("[{}] adding node with text: {:#?}", name, text);
                            target.insert_before(new_node);
                        }

                        new_nodes.len()
                    }
                })
                .unwrap_or_default();

            trace!("[{}] added {} items", name, count);
        }

        // write the file
        let mut buf = vec![];
        format_commonmark(root, self.options(), &mut buf).unwrap();
        let mut output_file = std::fs::File::create(&self.path())?;
        output_file.write_all(&buf)?;

        Ok(())
    }

    /// Add a new release to this WorkspaceChangelog.
    pub(crate) fn add_release(
        &'a self,
        title: String,
        crate_release_headings: &[WorkspaceCrateReleaseHeading<'a>],
    ) -> Fallible<()> {
        let root = self.root()?;

        let mut maybe_previous_release = None;
        let mut maybe_unreleased = None;

        // locate the previous release heading and unreleased heading if they exist
        for (i, node) in root.children().enumerate() {
            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                let mut msg = format!("[{}] heading at level {}", i, heading.level);

                if heading.level == WorkspaceChangelog::RELEASE_HEADING_LEVEL {
                    let heading_text = if let Some(heading_text) = get_heading_text(node) {
                        heading_text
                    } else {
                        continue;
                    };

                    msg += &format!(" => [{}] found heading text '{}'", i, heading_text);

                    if maybe_previous_release.is_none()
                        && !heading_text.to_lowercase().contains("changelog")
                    {
                        if heading_text.to_lowercase().contains("unreleased") {
                            msg += " => unreleased heading";
                            maybe_unreleased = Some(node);
                        } else {
                            msg += &format!(" => found previous release: {}", heading_text);
                            maybe_previous_release = Some(node);
                        }
                    }
                }

                trace!("{}", msg);
            }
        }

        // construct the new heading node
        let heading_value = NodeValue::Heading(comrak::nodes::NodeHeading {
            level: Self::RELEASE_HEADING_LEVEL,
            setext: false,
        });
        let heading_ast = comrak::nodes::Ast::new(heading_value);
        let heading = self
            .arena()
            .alloc(comrak::arena_tree::Node::new(core::cell::RefCell::new(
                heading_ast,
            )));

        let text_value = NodeValue::Text(title.into_bytes());
        let text_ast = comrak::nodes::Ast::new(text_value);
        let text = self
            .arena()
            .alloc(comrak::arena_tree::Node::new(core::cell::RefCell::new(
                text_ast,
            )));
        heading.append(text);

        // collect all new nodes for the new release heading and start with the heading itself
        let mut new_nodes: Vec<&'a comrak::arena_tree::Node<'a, RefCell<Ast>>> = vec![heading];

        if let Some(unreleased_node) = maybe_unreleased {
            // look for the crates that were released and remove their headings

            let release_crate_names = crate_release_headings
                .iter()
                .map(|wcrh| normalize_heading_name(&wcrh.prefix).to_lowercase())
                .collect::<HashSet<_>>();
            trace!(
                "will remove headings that match '{:?}'",
                release_crate_names
            );

            let mut remove_other = false;
            let mut first_heading_found = false;

            for (i, node) in unreleased_node.following_siblings().skip(1).enumerate() {
                let mut remove_nodes = vec![];

                if matches!(node.data.borrow().value, NodeValue::Heading(_)) {
                    first_heading_found = true;
                }

                match node.data.borrow().value {
                    NodeValue::Heading(found_heading_value)
                        if found_heading_value.level < CrateChangelog::RELEASE_HEADING_LEVEL =>
                    {
                        trace!(
                            "[{}] reached next workspace release heading '{:?}'. stopping search for crate heading. found? {}",
                            i,
                            get_heading_text(node),
                            !remove_nodes.is_empty()
                        );
                        break;
                    }

                    NodeValue::Heading(found_heading_value)
                        if found_heading_value.level == CrateChangelog::RELEASE_HEADING_LEVEL =>
                    {
                        if !remove_nodes.is_empty() {
                            trace!(
                                "[{}] reached next crate heading '{:?}'. stopping search for crate heading. found? {}",
                                i,
                                get_heading_text(node),
                                 !remove_nodes.is_empty()
                            );
                            break;
                        }

                        if let Some(heading_text) = get_heading_text(node) {
                            let normalized_heading_text = normalize_heading_name(&heading_text);
                            let res = release_crate_names.contains(&normalized_heading_text);
                            trace!(
                                "[{}] is '{}' to be removed? {}",
                                i,
                                &normalized_heading_text,
                                res
                            );
                            if res {
                                trace!(
                                    "[{:?}] removing unreleased crate heading '{}'",
                                    self.path(),
                                    heading_text,
                                );
                                remove_nodes.push(node);
                                remove_other = true;
                            } else {
                                remove_other = false;
                            }
                        }
                    }

                    _ if remove_other => remove_nodes.push(node),

                    _ if !first_heading_found => {
                        remove_nodes.push(node);
                        new_nodes.push(node);
                    }

                    _ => {}
                }

                for node in remove_nodes.iter().rev() {
                    node.detach();
                }
            }
        }

        // todo: add non-heading sibling items from the unreleased heading
        // if let Some(unreleased) = maybe

        // iterate over all crates and add their respective changes
        for WorkspaceCrateReleaseHeading {
            prefix,
            suffix: _,
            changelog,
        } in crate_release_headings.iter().rev()
        {
            let recent_release = changelog
                .topmost_release()?
                .ok_or_else(|| anyhow::anyhow!("expect {} to have a previous release", prefix))?
                .title()
                .to_owned();
            trace!(
                "[{:?}] looking for heading with text '{}'",
                changelog.path(),
                recent_release,
            );
            for node in changelog.root()?.children() {
                // we're only interested in the headings here
                if let NodeValue::Heading(found_heading_value) = node.data.borrow().value {
                    if found_heading_value.level == CrateChangelog::RELEASE_HEADING_LEVEL {
                        let found_heading_text = if let Some(text_str) = get_heading_text(node) {
                            text_str
                        } else {
                            continue;
                        };

                        trace!(
                            "[{:?}] found heading with level {}: {}",
                            changelog.path(),
                            found_heading_value.level,
                            found_heading_text,
                        );

                        if !found_heading_text.contains(&recent_release) {
                            continue;
                        }

                        trace!(
                            "[{:?}] found heading with text '{}', queueing for insertion...",
                            changelog.path(),
                            found_heading_text
                        );

                        {
                            // create and append the crate release heading for placement in the workspace changelog

                            let heading_value = NodeValue::Heading(comrak::nodes::NodeHeading {
                                level: CrateChangelog::RELEASE_HEADING_LEVEL,
                                setext: false,
                            });
                            let heading_ast = comrak::nodes::Ast::new(heading_value);
                            let heading_node = self.arena().alloc(comrak::arena_tree::Node::new(
                                core::cell::RefCell::new(heading_ast),
                            ));

                            let heading_text_value = NodeValue::Text(
                                format!("{}-{}", prefix, found_heading_text).into_bytes(),
                            );
                            let text_ast = comrak::nodes::Ast::new(heading_text_value);
                            let text_node = self.arena().alloc(comrak::arena_tree::Node::new(
                                core::cell::RefCell::new(text_ast),
                            ));

                            let link_value = NodeValue::Link(comrak::nodes::NodeLink {
                                // todo: derive this path dynamically
                                url: format!("crates/{}/CHANGELOG.md#{}", prefix, recent_release)
                                    .as_bytes()
                                    .to_vec(),
                                title: Default::default(),
                            });
                            let link_ast = comrak::nodes::Ast::new(link_value);
                            let link_node = self.arena().alloc(comrak::arena_tree::Node::new(
                                core::cell::RefCell::new(link_ast),
                            ));
                            link_node.append(text_node);
                            heading_node.append(link_node);

                            new_nodes.push(heading_node);
                        }

                        // add all siblings until the next release heading is reached
                        for sibling in node.following_siblings().skip(1) {
                            match sibling.data.borrow().value {
                                NodeValue::Heading(sibling_heading_value)
                                    if sibling_heading_value.level
                                        == CrateChangelog::RELEASE_HEADING_LEVEL =>
                                {
                                    break
                                }

                                _ => new_nodes.push(sibling),
                            };
                        }
                    }
                }
            }
        }

        for node in new_nodes {
            if let Some(previous_release) = maybe_previous_release {
                // by default try to insert before the previous release heading.
                trace!(
                    "adding before the top release '{}'",
                    get_nested_text(previous_release)
                );
                previous_release.insert_before(node);
            } else {
                // otherwise append at the end of the document.
                root.append(node);
            }
        }

        // write the file
        let mut buf = vec![];
        format_commonmark(root, self.options(), &mut buf).unwrap();
        let mut output_file = std::fs::File::create(&self.path())?;
        output_file.write_all(&buf)?;

        Ok(())
    }
}

fn get_nested_text<'a>(node: &'a comrak::arena_tree::Node<'a, RefCell<Ast>>) -> String {
    node.descendants().fold("".to_string(), |acc, node_l| {
        if let NodeValue::Text(ref text) = &node_l.data.borrow().value {
            acc + String::from_utf8_lossy(text).to_string().as_str()
        } else {
            acc
        }
    })
}

fn get_heading_text<'a>(node: &'a comrak::arena_tree::Node<'a, RefCell<Ast>>) -> Option<String> {
    node.descendants().skip(1).fold(None, |acc, node_l| {
        if let NodeValue::Text(ref text) = &node_l.data.borrow().value {
            let text_str = String::from_utf8_lossy(text).to_string();
            acc.map_or(Some(text_str.clone()), |v| Some(v + text_str.as_str()))
        } else {
            acc
        }
    })
}

/// Used to pass information about the new crate release headings to `WorkspaceChangelog::add_release`.
pub(crate) struct WorkspaceCrateReleaseHeading<'a> {
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) changelog: &'a ChangelogT<'a, CrateChangelog>,
}

impl<'a> WorkspaceCrateReleaseHeading<'a> {
    pub(crate) fn title(&self) -> String {
        format!("{}-{}", self.prefix, self.suffix)
    }
}

/// Applies an opinionated format to  a Markdown string.
pub(crate) fn sanitize(s: String) -> String {
    let arena = Arena::new();
    let mut options = ComrakOptions::default();
    options.parse.smart = true;
    options.extension.front_matter_delimiter = Some("---".to_owned());
    options.render.hardbreaks = true;

    let root = parse_document(&arena, &s, &options);
    let mut buf = vec![];
    format_commonmark(root, &options, &mut buf).unwrap();

    String::from_utf8(buf).unwrap()
}

fn print_node<'a>(
    node: &'a comrak::arena_tree::Node<'a, core::cell::RefCell<comrak::nodes::Ast>>,
    options: Option<ComrakOptions>,
) {
    let mut buf = vec![];
    format_commonmark(node, &options.unwrap_or_default(), &mut buf).unwrap();
    trace!("{}", String::from_utf8(buf).unwrap())
}

fn recursive_node_fn<'a, F>(
    node: &'a comrak::arena_tree::Node<'a, core::cell::RefCell<comrak::nodes::Ast>>,
    _reverse: bool,
    f: F,
) where
    F: Fn(&'a comrak::arena_tree::Node<'a, core::cell::RefCell<comrak::nodes::Ast>>),
{
    f(node);
    for d in node.children().skip(1) {
        f(d)
    }
}

fn recursive_detach<'a>(
    node: &'a comrak::arena_tree::Node<'a, core::cell::RefCell<comrak::nodes::Ast>>,
) {
    recursive_node_fn(node, false, |n| n.detach());
}

/// Implements the "aggregate" CLI subcommand.
pub(crate) fn cmd(
    args: &crate::cli::Args,
    cmd_args: &crate::cli::ChangelogArgs,
) -> crate::CommandResult {
    debug!("cmd_args: {:#?}", cmd_args);

    let ws = ReleaseWorkspace::try_new(args.workspace_path.clone())?;

    match &cmd_args.command {
        // todo: respect selection filter
        // todo: respect output path
        crate::cli::ChangelogCommands::Aggregate(_aggregate_args) => ws
            .changelog()
            .ok_or_else(|| anyhow::anyhow!("workspace doesn't have a changelog"))?
            .aggregate(ws.members()?)?,
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crate_selection::CrateStateFlags,
        tests::workspace_mocker::{example_workspace_1, example_workspace_1_aggregated_changelog},
    };
    use comrak::*;
    use enumflags2::make_bitflags;

    #[test]
    fn empty_frontmatter() {
        let workspace_mocker = example_workspace_1().unwrap();
        let changelog = ChangelogT::<WorkspaceChangelog>::at_path(
            &workspace_mocker.root().join("crates/crate_b/CHANGELOG.md"),
        );
        let fm: Result<Option<Frontmatter>, String> =
            changelog.front_matter().map_err(|e| e.to_string());

        assert_eq!(Ok(Some(Frontmatter::default())), fm);
    }

    #[test]
    fn no_frontmatter() {
        let workspace_mocker = example_workspace_1().unwrap();
        let changelog = ChangelogT::<WorkspaceChangelog>::at_path(
            &workspace_mocker.root().join("crates/crate_e/CHANGELOG.md"),
        );
        let fm: Result<Option<Frontmatter>, String> =
            changelog.front_matter().map_err(|e| e.to_string());

        assert_eq!(Ok(None), fm);
    }

    #[test]
    fn nonempty_frontmatter() {
        let workspace_mocker = example_workspace_1().unwrap();
        let changelog = ChangelogT::<WorkspaceChangelog>::at_path(
            &workspace_mocker.root().join("crates/crate_c/CHANGELOG.md"),
        );
        let fm: Result<Option<Frontmatter>, String> =
            changelog.front_matter().map_err(|e| e.to_string());

        assert_eq!(
            Ok(Some(Frontmatter {
                unreleasable: Some(true),
                default_unreleasable: Some(true),
                ..Default::default()
            })),
            fm
        );
    }

    #[test]
    fn workspace_changelog_aggregation() {
        let workspace_mocker = example_workspace_1().unwrap();

        let workspace = ReleaseWorkspace::try_new(workspace_mocker.root()).unwrap();

        let workspace_changelog = workspace.changelog().unwrap();
        workspace_changelog
            .aggregate(workspace.members().unwrap())
            .unwrap();

        let result = sanitize(std::fs::read_to_string(workspace_changelog.path()).unwrap());

        let expected = example_workspace_1_aggregated_changelog();

        assert_eq!(
            result,
            expected,
            "{}",
            prettydiff::text::diff_lines(&result, &expected).format()
        );
    }

    /// mock a release for crate_c and crate_e and the workspace, we expect the releasable
    /// crates to be removed from the Unreleased heading when aggregating the
    /// changelog.
    #[test]
    fn changelog_mock_release() {
        let workspace_mocker = example_workspace_1().unwrap();

        let workspace = ReleaseWorkspace::try_new_with_criteria(
            workspace_mocker.root(),
            crate::release::SelectionCriteria {
                match_filter: fancy_regex::Regex::new("^crate_(c|e)$").unwrap(),
                allowed_dev_dependency_blockers: make_bitflags!(CrateStateFlags::{MissingReadme}),
                allowed_selection_blockers: make_bitflags!(CrateStateFlags::{MissingReadme}),

                ..Default::default()
            },
        )
        .unwrap();
        let ws_changelog = workspace.changelog().unwrap();

        fn test_crate_changelog<'a>(
            workspace: &'a ReleaseWorkspace<'a>,
            name: &str,
            release_name: &str,
            expected: &str,
        ) -> WorkspaceCrateReleaseHeading<'a> {
            let cl = workspace
                .members()
                .unwrap()
                .iter()
                .find(|crt| crt.name() == name)
                .unwrap()
                .changelog()
                .unwrap();

            cl.add_release(String::from(release_name)).unwrap();

            let result = std::fs::read_to_string(cl.path()).unwrap();
            let expected = sanitize(String::from(expected));

            assert_eq!(
                result,
                expected,
                "\ndiff:\n{}",
                prettydiff::text::diff_lines(&result, &expected).format()
            );

            WorkspaceCrateReleaseHeading {
                prefix: String::from(name),
                suffix: String::from(release_name),
                changelog: cl,
            }
        }

        let crate_releases = vec![
            test_crate_changelog(
                &workspace,
                "crate_c",
                "0.0.1",
                indoc::indoc! {r#"
                ---
                unreleasable: true
                default_unreleasable: true
                ---
                # Changelog
                Hello

                ## [Unreleased]

                ## 0.0.1
                Awesome changes!

                ### Breaking
                Breaking changes, be careful.

                [Unreleased]: file:///dev/null
                "#
                },
            ),
            test_crate_changelog(
                &workspace,
                "crate_e",
                "0.0.1",
                indoc::indoc! {r#"
                # Changelog
                Hello. This crate is releasable.

                ## [Unreleased]

                ## 0.0.1
                Awesome changes!

                [Unreleased]: file:///dev/null
                "#
                },
            ),
        ];

        let release_name = "2021.mock";
        ws_changelog
            .add_release(release_name.to_string(), &crate_releases)
            .unwrap();

        let result = std::fs::read_to_string(ws_changelog.path()).unwrap();
        let expected = indoc::formatdoc!(
            r#"
            # Changelog
            This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released.
            The file is updated every time one or more crates are released.

            The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
            This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

            # [Unreleased]

            ## Something outdated maybe
            This will be removed by aggregation.


            ## [crate\_a](crates/crate_a/CHANGELOG.md#unreleased)
            ### Added
            - `InstallAppBundle`

            ## [crate\_f](crates/crate_f/CHANGELOG.md#unreleased)

            This will be released in the future.

            # {}
            The text beneath this heading will be retained which allows adding overarching release notes.

            ## [crate_e-0.0.1](crates/crate_e/CHANGELOG.md#0.0.1)
            Awesome changes!

            ## [crate_c-0.0.1](crates/crate_c/CHANGELOG.md#0.0.1)
            Awesome changes!

            ### Breaking
            Breaking changes, be careful.

            # [20210304.120604]
            This will include the hdk-0.0.100 release.

            ## [hdk-0.0.100](crates/hdk/CHANGELOG.md#0.0.100)

            ### Changed
            - hdk: fixup the autogenerated hdk documentation.
            "#,
            release_name
        );

        let expected = sanitize(expected);
        assert_eq!(
            result,
            expected,
            "\ndiff:\n{}",
            prettydiff::text::diff_lines(&result, &expected).format()
        );
    }

    #[test]
    fn find_crate_changes() {
        let workspace_mocker = example_workspace_1().unwrap();

        let inputs: &[(&str, PathBuf, Vec<ChangeT>)] = &[
            (
                "crate_a",
                workspace_mocker.root().join("crates/crate_a/CHANGELOG.md"),
                vec![
                    ChangeT::Unreleased,
                    ChangeT::Release(ReleaseChange::CrateReleaseChange("0.0.1".to_string())),
                ],
            ),
            (
                "crate_b",
                workspace_mocker.root().join("crates/crate_b/CHANGELOG.md"),
                vec![ChangeT::Unreleased],
            ),
            (
                "crate_c",
                workspace_mocker.root().join("crates/crate_c/CHANGELOG.md"),
                vec![ChangeT::Unreleased],
            ),
        ];

        for (name, changelog_path, expected_changes) in inputs {
            let changelog = ChangelogT::<CrateChangelog>::at_path(changelog_path);

            let changes = changelog.changes().unwrap();

            assert_eq!(expected_changes, &changes, "{}", name);
        }
    }

    #[test]
    fn find_workspace_changes() {
        let workspace_mocker = example_workspace_1().unwrap();

        let changelog_path = workspace_mocker.root().join("CHANGELOG.md");
        let changelog = ChangelogT::<WorkspaceChangelog>::at_path(&changelog_path);
        let changes = changelog.changes().unwrap();

        assert_eq!(
            vec![
                ChangeT::Changelog,
                ChangeT::Unreleased,
                ChangeT::Release(ReleaseChange::WorkspaceReleaseChange(
                    "[20210304.120604]".to_string(),
                    vec!["hdk-0.0.100".to_string()]
                )),
            ],
            changes
        );
    }

    use test_case::test_case;

    #[test_case(Frontmatter::default(), SemverIncrementMode::default())]
    #[test_case(Frontmatter{ semver_increment_mode: None, ..Default::default()}, SemverIncrementMode::default())]
    #[test_case(Frontmatter{ semver_increment_mode: Some(SemverIncrementMode::Minor), ..Default::default()}, SemverIncrementMode::Minor)]
    #[test_case(Frontmatter{ default_semver_increment_mode: Some(SemverIncrementMode::Minor), ..Default::default()}, SemverIncrementMode::Minor)]
    fn semver_increment_mode_getter(fm: Frontmatter, expected: SemverIncrementMode) {
        assert_eq!(fm.semver_increment_mode(), expected)
    }

    #[test]
    fn crate_changelog_reset_front_matter() {
        let workspace_mocker = example_workspace_1().unwrap();

        let read_changelog = move || -> ChangelogT<CrateChangelog> {
            ChangelogT::<CrateChangelog>::at_path(
                &workspace_mocker.root().join("crates/crate_a/CHANGELOG.md"),
            )
        };

        let cl = read_changelog();
        let fm_orig = cl.front_matter().unwrap().expect("expected fm initially");

        assert!(
            fm_orig.semver_increment_mode.is_some(),
            "expect semver_increment_mode initially"
        );

        cl.reset_front_matter_to_defaults().unwrap();

        let cl = read_changelog();
        let fm_new_readback = cl.front_matter().unwrap().unwrap();

        let fm_new_expected = Frontmatter {
            semver_increment_mode: None,

            ..fm_orig
        };
        assert_eq!(fm_new_expected, fm_new_readback);
    }
}
