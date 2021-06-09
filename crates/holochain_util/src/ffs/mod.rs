//! ffs - the Friendly Filesystem
//!
//! Wraps std::fs (or optionally, tokio::fs) in functions with identical
//! signatures such that error messages include extra context, in particular
//! the path used in the function call.
//!
//! This helps with "file not found" errors. Without ffs, the error would be:
//! ```ignore
//! Error: No such file or directory (os error 2)
//! ```
//!
//! and with ffs, the error becomes:
//! ```ignore
//! ffs::IoError at path '/foo/bar': No such file or directory (os error 2)
//! ```

mod io_error;

pub use self::io_error::{IoError, IoResult};
use std::path::PathBuf;

fn mapper<P: AsRef<std::path::Path>>(path: P) -> impl FnOnce(std::io::Error) -> IoError {
    move |e| IoError::new(e, path.as_ref().to_owned())
}

macro_rules! impl_ffs {
    ( $( fn $name:ident (path $(, $arg:ident : $arg_ty:ty)* ) -> $output:ty ; )* ) => {

        $(
            pub async fn $name<P: Clone + AsRef<std::path::Path>>(path: P $(, $arg : $arg_ty)*) -> IoResult<$output> {

                #[cfg(feature = "tokio")]
                return tokio::fs::$name(path.clone() $(, $arg)*).await.map_err(mapper(path));

                #[cfg(not(feature = "tokio"))]
                return std::fs::$name(path.clone() $(, $arg)*).map_err(mapper(path));
            }
        )*

        /// Wrap dunce::canonicalize, since std::fs::canonicalize has problems for Windows
        /// (see https://docs.rs/dunce/1.0.1/dunce/index.html)
        pub async fn canonicalize<P: Clone + AsRef<std::path::Path>>(path: P) -> IoResult<PathBuf> {
            dunce::canonicalize(path.clone()).map_err(mapper(path))
        }

        pub mod sync {

            use super::*;
            $(
                pub fn $name<P: Clone + AsRef<std::path::Path>>(path: P $(, $arg : $arg_ty)*) -> IoResult<$output> {
                    return std::fs::$name(path.clone() $(, $arg)*).map_err(mapper(path));
                }
            )*

            pub fn canonicalize<P: Clone + AsRef<std::path::Path>>(path: P) -> IoResult<PathBuf> {
                dunce::canonicalize(path.clone()).map_err(mapper(path))
            }

        }
    };
}

impl_ffs! {
    fn create_dir(path) -> ();
    fn create_dir_all(path) -> ();
    fn read(path) -> Vec<u8>;
    fn read_to_string(path) -> String;
    fn write(path, data: &[u8]) -> ();
}
