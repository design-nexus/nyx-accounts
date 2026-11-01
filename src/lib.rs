//! Native account authentication primitives. No GOA or EWS dependencies.
use oauth2::{CsrfToken, PkceCodeChallenge, PkceCodeVerifier};
use url::Url;

/// An outstanding authorization. Secrets deliberately have no Debug implementation.
pub struct Authorization {
    pub state: CsrfToken,
    pub verifier: PkceCodeVerifier,
    pub challenge: PkceCodeChallenge,
}
impl Default for Authorization {
    fn default() -> Self {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        Self {
            state: CsrfToken::new_random(),
            verifier,
            challenge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    MicrosoftPersonal,
}
impl Provider {
    pub fn authorization_endpoint(self) -> &'static str {
        match self {
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Self::MicrosoftPersonal => {
                "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize"
            }
        }
    }
    pub fn token_endpoint(self) -> &'static str {
        match self {
            Self::Google => "https://oauth2.googleapis.com/token",
            Self::MicrosoftPersonal => {
                "https://login.microsoftonline.com/consumers/oauth2/v2.0/token"
            }
        }
    }
}

/// Only explicit IPv4/IPv6 loopback literals may receive authorization codes.
pub fn valid_loopback_redirect(uri: &str) -> bool {
    let Ok(url) = Url::parse(uri) else {
        return false;
    };
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.query().is_none()
        && matches!(url.host(), Some(url::Host::Ipv4(ip)) if ip.is_loopback()
        )
        || valid_ipv6_loopback(uri)
}
fn valid_ipv6_loopback(uri: &str) -> bool {
    let Ok(url) = Url::parse(uri) else {
        return false;
    };
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.query().is_none()
        && matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_are_local_and_unambiguous() {
        for uri in [
            "http://127.0.0.1:4231/callback",
            "http://[::1]:4231/callback",
        ] {
            assert!(valid_loopback_redirect(uri));
        }
        for uri in [
            "https://example.com/callback",
            "http://localhost/callback",
            "http://0.0.0.0/callback",
            "http://user@127.0.0.1/callback",
            "http://127.0.0.1/#code",
        ] {
            assert!(!valid_loopback_redirect(uri));
        }
    }
    #[test]
    fn independent_authorizations_have_distinct_secrets() {
        let a = Authorization::default();
        let b = Authorization::default();
        assert_ne!(a.state.secret(), b.state.secret());
        assert_ne!(a.verifier.secret(), b.verifier.secret());
        assert_eq!(a.challenge.method().as_str(), "S256");
    }
}

pub mod client;
pub mod daemon;
pub mod model;
pub mod oauth;
pub mod providers;
pub mod store;

pub use client::Client;
pub use model::{Account, Service};

pub fn find_bin(name: &str) -> Option<std::path::PathBuf> {
    let current = std::env::current_exe().ok()?.with_file_name(name);
    current.is_file().then_some(current)
}
pub fn open_management() -> anyhow::Result<()> {
    let settings = find_bin("nyx-settings")
        .ok_or_else(|| anyhow::anyhow!("Preview Settings is not installed"))?;
    std::process::Command::new(settings)
        .args(["--section", "online-accounts"])
        .spawn()?;
    Ok(())
}
pub mod remote;

/// Keep service lifetime bound to its owning bus, including an unexpectedly stopped preview session.
pub async fn wait_for_bus_disconnect(connection: zbus::Connection) {
    use futures_util::StreamExt;
    let mut messages = zbus::MessageStream::from(connection);
    while let Some(message) = messages.next().await {
        if message.is_err() {
            break;
        }
    }
}

/// Prevent an old bus generation from overlapping a newly activated service during cleanup.
pub fn service_lifetime_lock(path: &std::path::Path) -> anyhow::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let guard = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    guard.try_lock().map_err(|_| {
        anyhow::anyhow!("The previous preview service is still shutting down; retry shortly")
    })?;
    Ok(guard)
}

#[cfg(test)]
mod lifetime_tests {
    #[test]
    fn service_generation_lock_is_released_only_when_owner_exits() {
        let path = std::env::temp_dir().join(format!("nyx-lifetime-{}", uuid::Uuid::new_v4()));
        let guard = super::service_lifetime_lock(&path).unwrap();
        assert!(super::service_lifetime_lock(&path).is_err());
        drop(guard);
        assert!(super::service_lifetime_lock(&path).is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
