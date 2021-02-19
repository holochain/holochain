#[derive(Debug)]
pub struct IoError {
    pub(crate) original: std::io::Error,
    pub(crate) path: Option<std::path::PathBuf>,
    #[cfg(feature = "backtrace")]
    pub(crate) backtrace: backtrace::Backtrace,
}

pub type IoResult<T> = Result<T, IoError>;

impl std::error::Error for IoError {}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = if let Some(path) = &self.path {
            path.to_string_lossy()
        } else {
            "(unknown path)".into()
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "backtrace")] {
                write!(
                    f,
                    "ffs::IoError at path '{}': {}\nbacktrace:\n{:?}",
                    path, self.original, self.backtrace
                )
            } else {
                write!(
                    f,
                    "ffs::IoError at path '{}': {}",
                    path, self.original
                )
            }
        }
    }
}

impl IoError {
    pub fn into_inner(self) -> std::io::Error {
        self.original
    }

    pub fn new(original: std::io::Error, path: std::path::PathBuf) -> Self {
        let path = Some(path);
        cfg_if::cfg_if! {
            if #[cfg(feature = "backtrace")] {
                Self {original, path, backtrace: backtrace::Backtrace::new() }
            } else {
                Self {original, path }
            }
        }
    }
}
