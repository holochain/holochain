/// The anchor pattern implemented in terms of [ `path::Path` ]
///
/// The anchor pattern predates the path crate.
///
/// It is conceptually:
///
/// - A two level Path tree
/// - Each level of the path is defined as strings not binary data
/// - The top level is the "type" and the second level is the "text"
/// - The second level is optional as `Option<String>`
pub mod anchor;

/// The generic [ `path::Path` ] pattern.
///
/// As explained in the parent module documentation the [ `path::Path` ] defines a tree structure.
///
/// The path is _not_ an entire tree but simply one path from the root to the current depth of the tree.
///
/// A -> B -> C
///  \-> D
///
/// All possible paths for the above tree:
///
/// - `[]`
/// - `[ A ]`
/// - `[ A B ]`
/// - `[ A B C ]`
/// - `[ A D ]`
///
/// Note:
///
/// - The path must always start at the root
/// - [ `path::Path` ] are sequential and contigious
/// - [ `path::Path` ] may be empty
/// - [ `path::Path` ] only track one branch
///
/// Applications can discover all links from a path to all children by constructing the known path components.
///
/// For example if an application knows `[ A ]` then links to `B` and `D` will be discoverable.
///
/// If an application knows `[ A B ]` then a link to `C` will be discoverable.
pub mod path;
