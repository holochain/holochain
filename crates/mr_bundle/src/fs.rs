use std::path::Path;

pub async fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::read(path).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::read(path);
}

pub async fn write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::write(path, data).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::write(path, data);
}
