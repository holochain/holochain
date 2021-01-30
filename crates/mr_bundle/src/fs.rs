use std::path::Path;

use crate::io_error::{IoError, IoResult};

pub fn fs<'a>(path: &'a Path) -> Fs<'a> {
    Fs { path }
}
pub struct Fs<'a> {
    path: &'a Path,
}

impl<'a> Fs<'a> {
    pub async fn read(&self) -> IoResult<Vec<u8>> {
        #[cfg(feature = "tokio-io")]
        return self.map_err(tokio::fs::read(self.path).await);

        #[cfg(not(feature = "tokio-io"))]
        return self.map_err(std::fs::read(self.path));
    }

    pub async fn read_to_string(&self) -> IoResult<String> {
        #[cfg(feature = "tokio-io")]
        return self.map_err(tokio::fs::read_to_string(self.path).await);

        #[cfg(not(feature = "tokio-io"))]
        return self.map_err(std::fs::read_to_string(self.path));
    }

    pub async fn write(&self, data: &[u8]) -> IoResult<()> {
        #[cfg(feature = "tokio-io")]
        return self.map_err(tokio::fs::write(self.path, data).await);

        #[cfg(not(feature = "tokio-io"))]
        return self.map_err(std::fs::write(self.path, data));
    }

    pub async fn create_dir_all(&self) -> IoResult<()> {
        #[cfg(feature = "tokio-io")]
        return self.map_err(tokio::fs::create_dir_all(self.path).await);

        #[cfg(not(feature = "tokio-io"))]
        return self.map_err(std::fs::create_dir_all(self.path));
    }

    fn map_err<T>(&self, err: std::io::Result<T>) -> IoResult<T> {
        err.map_err(|e| IoError::new(e, Some(self.path.to_owned())))
    }
}
