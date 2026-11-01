use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Service {
    Mail,
    Calendar,
    Contacts,
    Files,
    Tasks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub provider: String,
    pub identity: String,
    pub label: String,
    pub enabled: BTreeSet<Service>,
    pub status: String,
    pub client_id: Option<String>,
    #[serde(default)]
    pub endpoints: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub legacy_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub provider_user_id: Option<String>,
}
impl Account {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.identity.trim().is_empty(),
            "An account identity is required"
        );
        anyhow::ensure!(
            matches!(
                self.provider.as_str(),
                "google"
                    | "microsoft_personal"
                    | "fastmail"
                    | "zoho"
                    | "nextcloud"
                    | "imap_smtp"
                    | "caldav"
                    | "carddav"
                    | "webdav"
            ),
            "Unsupported provider"
        );
        anyhow::ensure!(
            self.enabled
                .is_subset(&crate::providers::capabilities(&self.provider)?.services),
            "Provider does not support the selected services"
        );
        for (kind, endpoint) in &self.endpoints {
            anyhow::ensure!(
                matches!(
                    kind.as_str(),
                    "imap" | "smtp" | "caldav" | "carddav" | "webdav"
                ),
                "Unsupported endpoint type"
            );
            let uri = url::Url::parse(endpoint)?;
            anyhow::ensure!(
                uri.username().is_empty()
                    && uri.password().is_none()
                    && uri.fragment().is_none()
                    && uri.query().is_none(),
                "Endpoints must not contain credentials or fragments"
            );
            let secure = match kind.as_str() {
                "imap" => matches!(uri.scheme(), "imaps" | "imap+starttls"),
                "smtp" => matches!(uri.scheme(), "smtps" | "smtp+starttls"),
                _ => uri.scheme() == "https",
            };
            anyhow::ensure!(
                secure && uri.host_str().is_some(),
                "A secure endpoint is required"
            );
        }
        Ok(())
    }
}

/// Secret material never implements Debug and is wiped on drop.
#[derive(Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub password: String,
    pub client_secret: String,
    pub expires_at: u64,
}
impl Credentials {
    pub fn empty() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: String::new(),
            password: String::new(),
            client_secret: String::new(),
            expires_at: 0,
        }
    }
}

impl Account {
    pub fn label(&self) -> &str {
        if self.label.is_empty() {
            &self.identity
        } else {
            &self.label
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Account {
        Account {
            id: String::new(),
            provider: "imap_smtp".into(),
            identity: "fixture@example.test".into(),
            label: "Fixture".into(),
            enabled: [Service::Mail].into_iter().collect(),
            status: String::new(),
            client_id: None,
            endpoints: [("imap".into(), "imaps://mail.example.test:993".into())]
                .into_iter()
                .collect(),
            legacy_id: None,
            error: None,
            provider_user_id: None,
        }
    }
    #[test]
    fn endpoint_credentials_and_plaintext_are_rejected() {
        for endpoint in [
            "imap://mail.example.test",
            "imaps://user:password@mail.example.test",
            "imaps://mail.example.test?token=fixture",
        ] {
            let mut a = fixture();
            a.endpoints.insert("imap".into(), endpoint.into());
            assert!(a.validate().is_err());
        }
    }
    #[test]
    fn unsupported_service_cannot_be_enabled() {
        let mut a = fixture();
        a.enabled.insert(Service::Files);
        assert!(a.validate().is_err());
    }
    #[test]
    fn unknown_secret_fields_do_not_enter_metadata() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["refresh_token"] = serde_json::json!("fixture-secret");
        let account: Account = serde_json::from_value(value).unwrap();
        assert!(
            !serde_json::to_string(&account)
                .unwrap()
                .contains("fixture-secret")
        );
    }
}
