#[derive(Debug, thiserror::Error, derive_more::Constructor)]
pub struct IoError(std::io::Error, Option<std::path::PathBuf>);

pub type IoResult<T> = Result<T, IoError>;

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = if let Some(path) = &self.1 {
            path.to_string_lossy()
        } else {
            "(unknown path)".into()
        };
        write!(f, "{}: {}", path, self.0)
    }
}

impl From<std::io::Error> for IoError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err, None)
    }
}

impl IoError {
    pub fn into_inner(self) -> std::io::Error {
        self.0
    }
}

// #[macro_export]
// macro_rules! impl_map_io_error {
//     ($err_type: ident) => {
//         impl $err_type {
//             pub fn map_io_err(path: PathBuf) -> impl (FnOnce(std::io::Error) -> $err_type) {
//                 |err| $err_type::IoError($crate::io_error::IoError::new(err, path))
//             }
//         }
//     };
// }
