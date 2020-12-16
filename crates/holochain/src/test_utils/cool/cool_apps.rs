use super::CoolCell;
use holochain_types::app::InstalledAppId;
use itertools::traits::HomogeneousTuple;

/// Return type of opinionated setup function
pub struct CoolApps(pub(super) CoolAppsRaw);

type CoolAppsRaw = Vec<(InstalledAppId, Vec<CoolCell>)>;

impl CoolApps {
    /// Get the underlying data
    pub fn into_inner(self) -> CoolAppsRaw {
        self.0
    }

    /// Helper to destructure the nested app setup return value as nested tuples.
    /// Each level of nesting can contain 1-4 items, i.e. up to 4 agents with 4 DNAs each.
    /// Beyond 4, and this will PANIC! (But it's just for tests so it's fine.)
    pub fn into_tuples<Outer, Inner>(self) -> Outer
    where
        Outer: HomogeneousTuple<Item = Inner>,
        Inner: HomogeneousTuple<Item = CoolCell>,
        Outer::Buffer: std::convert::AsRef<[Option<Inner>]>,
        Outer::Buffer: std::convert::AsMut<[Option<Inner>]>,
        Inner::Buffer: std::convert::AsRef<[Option<CoolCell>]>,
        Inner::Buffer: std::convert::AsMut<[Option<CoolCell>]>,
    {
        use itertools::Itertools;
        self.into_inner()
            .into_iter()
            .map(|(_, v)| {
                v.into_iter()
                    .collect_tuple::<Inner>()
                    .expect("Can't destructure more than 4 DNAs")
            })
            .collect_tuple::<Outer>()
            .expect("Can't destructure more than 4 Agents")
    }
}

#[macro_export]
macro_rules! destructure_test_cell_vec {
    ($vec:expr) => {{
        use itertools::Itertools;
        let vec: Vec<$crate::test_utils::cool::CoolApps> = $vec;
        vec.into_iter()
            .map(|blob| blob.into_tuples())
            .collect_tuple()
            .expect("Can't destructure more than 4 Conductors")
    }};
}
