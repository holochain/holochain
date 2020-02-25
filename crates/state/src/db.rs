
pub enum DbName {
    ChainEntries,
    ChainHeaders,
    ChainMeta,
}

impl std::fmt::Display for DbName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DbName::*;
        match self {
            ChainEntries => write!(f, "chain-entries"),
            ChainHeaders => write!(f, "chain-headers"),
            ChainMeta => write!(f, "chain-meta"),
        }
    }
}
