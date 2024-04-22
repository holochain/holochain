use holochain_conductor_api::AppAuthenticationToken;
use rand::RngCore;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::SystemTime;

use crate::conductor::error::{ConductorError, ConductorResult};
use holochain_types::prelude::InstalledAppId;

pub struct AppAuthTokenStore {
    issued_tokens: HashMap<AppAuthenticationToken, TokenMeta>,
}

impl AppAuthTokenStore {
    pub fn new() -> Self {
        Self {
            issued_tokens: HashMap::new(),
        }
    }

    /// Issue a token that can be used to authenticate a connection. The token will only be valid
    /// for use with the specified `installed_app_id` and will expire after `expiry_seconds`.
    ///
    /// If `single_use` is true, the token will be invalidated after the first use, successful or not.
    pub fn issue_token(
        &mut self,
        installed_app_id: InstalledAppId,
        expiry_seconds: u64,
        single_use: bool,
    ) -> (AppAuthenticationToken, Option<SystemTime>) {
        let mut token = [0u8; 64];
        rand::thread_rng().fill_bytes(&mut token);
        let token = token.to_vec();

        let expires_at =
            SystemTime::now().checked_add(std::time::Duration::from_secs(expiry_seconds));
        self.issued_tokens.insert(
            token.clone(),
            TokenMeta {
                installed_app_id,
                expires_at,
                single_use,
            },
        );
        self.remove_expired_tokens();

        (token, expires_at)
    }

    /// Authenticate a token and return the `InstalledAppId` that the token was issued for.
    ///
    /// If `app_id_restriction` is provided, the token will only be valid for the specified `InstalledAppId`.
    /// This is useful when an app interface is restricted to a single app and tokens that would
    /// otherwise be valid, are not valid for connecting to this app interface.
    pub fn authenticate_token(
        &mut self,
        token: AppAuthenticationToken,
        app_id_restriction: Option<InstalledAppId>,
    ) -> ConductorResult<InstalledAppId> {
        self.remove_expired_tokens();

        match self.issued_tokens.entry(token) {
            Entry::Occupied(entry) => {
                let meta = { entry.get().clone() };

                if meta.single_use {
                    entry.remove();
                }

                if let Some(app_id_restriction) = app_id_restriction {
                    if app_id_restriction != meta.installed_app_id {
                        return Err(ConductorError::FailedAuthenticationError(
                            "Attempt to use token in the context of another application"
                                .to_string(),
                        ));
                    }
                }

                let app_id = meta.installed_app_id.clone();

                Ok(app_id)
            }
            Entry::Vacant(_) => Err(ConductorError::FailedAuthenticationError(
                "Invalid token".to_string(),
            )),
        }
    }

    fn remove_expired_tokens(&mut self) {
        let current_time = SystemTime::now();

        self.issued_tokens.retain(|_, meta| {
            if let Some(expires_at) = meta.expires_at {
                expires_at > current_time
            } else {
                // Always keep tokens that are set to not expire
                true
            }
        });
    }

    #[cfg(test)]
    fn age_tokens(&mut self) {
        self.issued_tokens.iter_mut().for_each(|(_, meta)| {
            meta.expires_at = Some(
                SystemTime::now()
                    .checked_sub(std::time::Duration::from_secs(10))
                    .unwrap(),
            );
        });
    }

    #[cfg(test)]
    fn get_tokens(&self) -> &HashMap<AppAuthenticationToken, TokenMeta> {
        &self.issued_tokens
    }
}

impl Default for AppAuthTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct TokenMeta {
    installed_app_id: InstalledAppId,
    expires_at: Option<SystemTime>,
    single_use: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn issue_and_use_single_use_token() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 10, true);

        let authenticated_for_app = auth.authenticate_token(token.clone(), None).unwrap();
        assert_eq!(authenticated_for_app, installed_app_id);

        let result = auth.authenticate_token(token.clone(), None);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reuse_token() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 10, false);

        let authenticated_for_app = auth.authenticate_token(token.clone(), None).unwrap();
        assert_eq!(authenticated_for_app, installed_app_id);

        let authenticated_for_app = auth.authenticate_token(token.clone(), None).unwrap();
        assert_eq!(authenticated_for_app, installed_app_id);
    }

    #[tokio::test]
    async fn attempt_with_token_that_does_not_exist() {
        let mut auth = AppAuthTokenStore::new();
        let result = auth.authenticate_token(vec![0; 16], None);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn use_token_with_app_restriction() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 1, true);

        let result = auth.authenticate_token(token.clone(), Some(installed_app_id.clone()));
        assert_eq!(result.unwrap(), installed_app_id);
    }

    #[tokio::test]
    async fn use_token_with_app_restriction_mismatch() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 1, true);

        let other_app_id = "other_app".to_string();
        let result = auth.authenticate_token(token.clone(), Some(other_app_id));
        assert!(result.is_err());

        // Token was invalidated by the use in a failed attempt
        let result = auth.authenticate_token(token.clone(), Some(installed_app_id.clone()));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn use_token_with_app_restriction_mismatch_multi_use() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 1, false);

        let other_app_id = "other_app".to_string();
        let result = auth.authenticate_token(token.clone(), Some(other_app_id));
        assert!(result.is_err());

        // Token was retained through the failed attempt because the caller has used it with a
        // websocket connection that is restricted to another app.
        let result = auth.authenticate_token(token.clone(), Some(installed_app_id.clone()));
        assert_eq!(result.unwrap(), installed_app_id);
    }

    #[tokio::test]
    async fn use_expired_token() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        let (token, _) = auth.issue_token(installed_app_id.clone(), 1, true);

        auth.age_tokens();

        let result = auth.authenticate_token(token.clone(), None);
        assert!(result.is_err());

        assert!(auth.get_tokens().is_empty());
    }

    #[tokio::test]
    async fn issuing_new_tokens_removes_expired_tokens() {
        let mut auth = AppAuthTokenStore::new();
        let installed_app_id = "test_app".to_string();
        for _ in 0..3 {
            auth.issue_token(installed_app_id.clone(), 1, true);
        }

        assert_eq!(3, auth.get_tokens().len());
        auth.age_tokens();
        // Just to show that the test code didn't mess with the state
        assert_eq!(3, auth.get_tokens().len());

        // Having this clear out the expired tokens means that even if a client is issuing tokens that
        // don't get used, older tokens will still be dropped.
        let (token, _) = auth.issue_token(installed_app_id.clone(), 1, true);

        assert_eq!(1, auth.get_tokens().len());
        assert_eq!(token, *auth.get_tokens().iter().next().unwrap().0);
    }
}
