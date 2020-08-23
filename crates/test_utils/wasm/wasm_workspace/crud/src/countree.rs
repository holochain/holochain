use hdk3::prelude::*;

#[hdk_entry(id = "countree")]
/// a tree of counters
#[derive(Default, Clone, Copy)]
pub struct CounTree(u32);

impl std::ops::Add for CounTree {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl CounTree {
    pub fn new() -> ExternResult<HeaderHash> {
        Self::ensure(Self::default())
    }

    pub fn ensure(countree: CounTree) -> ExternResult<HeaderHash> {
        Ok(commit_entry!(countree)?)
    }

    pub fn get_or_new(header_hash: HeaderHash) -> ExternResult<CounTree> {
        let maybe: Option<Element> = get!(header_hash)?;
        match maybe {
            Some(element) => match element.entry().to_app_option()? {
                Some(v) => Ok(v),
                None => Ok(Self::get_or_new(Self::new()?)?),
            },
            None => Ok(Self::get_or_new(Self::new()?)?),
        }
    }

    pub fn details(header_hash: HeaderHash) -> ExternResult<GetDetailsOutput> {
        Ok(GetDetailsOutput::new(get_details!(header_hash)?))
    }

    pub fn inc(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
        Ok(Self::ensure(Self::get_or_new(header_hash)? + CounTree(1))?)
    }

    pub fn dec(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
        Ok(delete_entry!(header_hash)?)
    }
}
