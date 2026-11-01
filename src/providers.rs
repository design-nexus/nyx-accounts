//! Provider capability and standard-protocol discovery, without synchronization.
use crate::model::{Account, Service};
use anyhow::{Context, Result};
use std::collections::BTreeSet;

pub struct Capabilities {
    pub services: BTreeSet<Service>,
    pub app_password: bool,
}
pub fn capabilities(provider: &str) -> Result<Capabilities> {
    use Service::*;
    let (services, app_password): (&[Service], bool) = match provider {
        "google" | "microsoft_personal" => (&[Mail, Calendar, Contacts, Files, Tasks], false),
        "fastmail" | "zoho" => (&[Mail, Calendar, Contacts], true),
        "nextcloud" => (&[Calendar, Contacts, Files, Tasks], true),
        "imap_smtp" => (&[Mail], true),
        "caldav" => (&[Calendar, Tasks], true),
        "carddav" => (&[Contacts], true),
        "webdav" => (&[Calendar, Contacts, Files, Tasks], true),
        _ => anyhow::bail!("Unsupported provider"),
    };
    Ok(Capabilities {
        services: services.iter().cloned().collect(),
        app_password,
    })
}
/// Presets are hints; successful protocol discovery is still required.
pub fn apply_presets(account: &mut Account, region: Option<&str>) -> Result<()> {
    if account.provider == "fastmail" {
        for (kind, endpoint) in [
            ("imap", "imaps://imap.fastmail.com:993"),
            ("smtp", "smtps://smtp.fastmail.com:465"),
            ("caldav", "https://caldav.fastmail.com/dav/"),
            ("carddav", "https://carddav.fastmail.com/dav/"),
        ] {
            account
                .endpoints
                .entry(kind.into())
                .or_insert_with(|| endpoint.into());
        }
    }
    if account.provider == "google" {
        account
            .endpoints
            .entry("imap".into())
            .or_insert_with(|| "imaps://imap.gmail.com:993".into());
        account
            .endpoints
            .entry("smtp".into())
            .or_insert_with(|| "smtps://smtp.gmail.com:465".into());
    }
    if account.provider == "zoho" {
        let region = region.unwrap_or("zoho.com");
        anyhow::ensure!(
            [
                "zoho.com",
                "zoho.eu",
                "zoho.in",
                "zoho.com.au",
                "zoho.jp",
                "zohocloud.ca",
                "zoho.sa",
                "zoho.com.cn"
            ]
            .contains(&region),
            "Unsupported Zoho region"
        );
        let domain = account
            .identity
            .rsplit('@')
            .next()
            .context("Email is required")?;
        let suffix = if domain == "zoho.com" || domain == "zohomail.com" {
            ""
        } else {
            "pro"
        };
        for (kind, endpoint) in [
            ("imap", format!("imaps://imap{suffix}.{region}:993")),
            ("smtp", format!("smtps://smtp{suffix}.{region}:465")),
        ] {
            account.endpoints.entry(kind.into()).or_insert(endpoint);
        }
        // DAV availability and URLs vary by plan/region. Never invent them.
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_provider_cannot_claim_unrelated_services() {
        assert!(
            !capabilities("carddav")
                .unwrap()
                .services
                .contains(&Service::Mail)
        );
        assert!(capabilities("exchange").is_err());
    }
}
