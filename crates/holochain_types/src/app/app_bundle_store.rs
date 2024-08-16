use super::*;

/// Something that stores AppBundles and knows how to retrieve them by InstalledAppId.
#[async_trait::async_trait]
pub trait AppBundleStore {
    /// Resolve the app bundle source to a bundle.
    async fn get_app_bundle(&self, app_id: &InstalledAppId) -> Result<AppBundle, AppBundleError>;
}
