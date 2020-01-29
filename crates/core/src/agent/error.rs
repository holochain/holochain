

#[derive(Debug)]
pub enum SourceChainError {
    ChainNotInitialized,
    MissingHead,
}

impl std::error::Error for SourceChainError {}

impl std::fmt::Display for SourceChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}


pub type SourceChainResult<T> = Result<T, SourceChainError>;
