
#[derive(Debug)]
pub enum DbName {
    ChainEntries,
    ChainHeaders,
    ChainMeta,
    ChainSequence,
}

impl std::fmt::Display for DbName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DbName::*;
        match self {
            ChainEntries => write!(f, "ChainEntries"),
            ChainHeaders => write!(f, "ChainHeaders"),
            ChainMeta => write!(f, "ChainMeta"),
            ChainSequence => write!(f, "ChainSequence"),
        }
    }
}
