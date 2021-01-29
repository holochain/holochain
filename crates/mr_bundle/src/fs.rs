use std::path::Path;

pub async fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::read(path).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::read(path);
}

pub async fn read_to_string(path: &Path) -> std::io::Result<String> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::read_to_string(path).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::read_to_string(path);
}

pub async fn write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::write(path, data).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::write(path, data);
}

pub async fn create_dir_all(path: &Path) -> std::io::Result<()> {
    #[cfg(feature = "tokio-io")]
    return tokio::fs::create_dir_all(path).await;

    #[cfg(not(feature = "tokio-io"))]
    return std::fs::create_dir_all(path);
}
