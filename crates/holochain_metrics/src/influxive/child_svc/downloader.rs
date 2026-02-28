#![deny(missing_docs)]
#![deny(unsafe_code)]
//! Influxive system download utility. It's probably not useful to use this
//! crate directly. It mainly exists as separate from the
//! influxive-child-svc crate as a means to make it easy for the dependencies
//! to be optional.

use super::super::types::{InfluxiveError, InfluxiveResult};

/// Indicate what archive type is used in the target.
#[derive(Clone, Debug)]
pub enum Archive {
    /// gzip tar archive
    #[cfg(not(target_os = "windows"))]
    TarGz {
        /// Path inside archive to target file.
        // str instead of Path so it can be const initialized
        inner_path: &'static str,
    },

    /// zip archive
    #[cfg(target_os = "windows")]
    Zip {
        /// Path inside archive to target file.
        // str instead of Path so it can be const initialized
        inner_path: &'static str,
    },
}

/// Indicate the hash type to verify.
#[derive(Debug, Clone)]
pub enum Hash {
    /// A sha2 256 bit hash.
    Sha2_256(&'static [u8; 32]),
}

trait Hasher: 'static + digest::DynDigest + std::io::Write + Send {}

impl Hasher for sha2::Sha256 {}

impl Hash {
    fn as_slice(&self) -> &[u8] {
        match self {
            Hash::Sha2_256(b) => &b[..],
        }
    }

    fn get_hasher(&self) -> Box<dyn Hasher + 'static + Send> {
        use sha2::Digest;
        Box::new(sha2::Sha256::new())
    }
}

/// Specification for download.
#[derive(Clone, Debug)]
pub struct DownloadSpec {
    /// Remote url for download.
    pub url: &'static str,

    /// The archive definition.
    pub archive: Archive,

    /// The hash of the whole archive file.
    pub archive_hash: Hash,

    /// The target file prefix.
    pub file_prefix: &'static str,

    /// The target file extension.
    pub file_extension: &'static str,
}

impl DownloadSpec {
    /// Check the local system for a cached version of this download,
    /// if found, return that path. Otherwise download, unpack, and
    /// verify, returning that newly downloaded path.
    pub async fn download(&self, fallback_path: &std::path::Path) -> InfluxiveResult<std::path::PathBuf> {
        let name = format!("{}-{}", self.file_prefix, self.file_extension,);

        let cache_path = dirs::data_local_dir().map(|mut d| {
            d.push(&name);
            d
        });

        let cache_path = if let Some(cache_path) = cache_path {
            if let Ok(true) = tokio::fs::try_exists(&cache_path).await {
                return Ok(cache_path);
            }
            Some(cache_path)
        } else {
            cache_path
        };

        let mut fallback_path = fallback_path.to_owned();
        fallback_path.push(name);

        if let Ok(true) = tokio::fs::try_exists(&fallback_path).await {
            return Ok(fallback_path);
        }

        let (_tmp, dl_path) = self.extract().await?;

        if let Some(cache_path) = cache_path {
            if let Ok(()) = tokio::fs::rename(&dl_path, &cache_path).await {
                return Ok(cache_path);
            }
        }

        tokio::fs::rename(&dl_path, &fallback_path).await?;
        Ok(fallback_path)
    }

    async fn extract(&self) -> InfluxiveResult<(tempfile::TempDir, std::path::PathBuf)> {
        use futures::stream::StreamExt;
        use tokio::io::AsyncSeekExt;
        use tokio::io::AsyncWriteExt;

        let tmp = tempfile::tempdir()?;

        let file = tempfile::tempfile()?;
        let mut file = tokio::fs::File::from_std(file);

        let response = reqwest::get(self.url).await?;
        if !response.status().is_success() {
            return Err(InfluxiveError::DownloadFailed(response.status().as_u16()));
        }

        let mut data_stream = response.bytes_stream();
        let mut hasher = self.archive_hash.get_hasher();

        while let Some(bytes) = data_stream.next().await {
            let bytes = bytes?;
            hasher.update(&bytes);

            let mut reader: &[u8] = &bytes;
            tokio::io::copy(&mut reader, &mut file).await?;
        }

        let hash = hasher.finalize();
        if &*hash != self.archive_hash.as_slice() {
            return Err(InfluxiveError::HashMismatch {
                expected: hex::encode(self.archive_hash.as_slice()),
                actual: hex::encode(hash),
            });
        }

        file.flush().await?;
        file.rewind().await?;
        let file = file.into_std().await;

        let inner_path = match &self.archive {
            #[cfg(not(target_os = "windows"))]
            Archive::TarGz { inner_path } => {
                self.extract_tar_gz(tmp.path().to_owned(), file).await?;
                inner_path
            }
            #[cfg(target_os = "windows")]
            Archive::Zip { inner_path } => {
                self.extract_zip(tmp.path().to_owned(), file).await?;
                inner_path
            }
        };

        let mut tgt = tmp.path().to_owned();
        tgt.push(inner_path);

        Ok((tmp, tgt))
    }

    #[cfg(not(target_os = "windows"))]
    async fn extract_tar_gz(&self, tmp: std::path::PathBuf, mut src: std::fs::File) -> std::io::Result<()> {
        tokio::task::spawn_blocking(move || {
            use std::io::Seek;
            use std::io::Write;

            let big_file = tempfile::tempfile()?;
            let mut decoder = flate2::write::GzDecoder::new(big_file);
            std::io::copy(&mut src, &mut decoder)?;
            let mut big_file = decoder.finish()?;
            big_file.flush()?;
            big_file.rewind()?;

            let mut archive = tar::Archive::new(big_file);
            archive.unpack(tmp)
        })
        .await?
    }

    #[cfg(target_os = "windows")]
    async fn extract_zip(&self, tmp: std::path::PathBuf, src: std::fs::File) -> std::io::Result<()> {
        tokio::task::spawn_blocking(move || {
            let mut archive = zip::ZipArchive::new(src).map_err(std::io::Error::other)?;
            archive.extract(tmp).map_err(std::io::Error::other)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "windows"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn tar_gz_sanity() {
        let tar = crate::influxive::child_svc::download_binaries::DL_CLI.unwrap();

        println!("{tar:?}");

        let mut all = Vec::new();
        for _ in 0..2 {
            let tar = tar.clone();
            all.push(tokio::task::spawn(async move {
                let tmp = tempfile::tempdir().unwrap();

                println!("{:?}", tar.download(tmp.path()).await.unwrap());

                // okay if windows fails
                let _ = tmp.close();
            }));
        }

        for task in all {
            task.await.unwrap();
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test(flavor = "multi_thread")]
    async fn zip_sanity() {
        let zip = crate::child_svc::download_binaries::DL_CLI.unwrap();

        println!("{zip:?}");

        let mut all = Vec::new();
        for _ in 0..2 {
            let zip = zip.clone();
            all.push(tokio::task::spawn(async move {
                let tmp = tempfile::tempdir().unwrap();

                println!("{:?}", zip.download(tmp.path()).await.unwrap());

                // okay if windows fails
                let _ = tmp.close();
            }));
        }

        for task in all {
            task.await.unwrap();
        }
    }
}
