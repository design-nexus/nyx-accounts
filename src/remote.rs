//! Protocol adapters used by application engines. The accounts daemon never syncs data.
use crate::model::{Account, Credentials, Service};
use anyhow::{Context, Result};
use reqwest::{
    Method,
    blocking::{Client, RequestBuilder},
};
use serde_json::Value;
use std::io::Read;
use url::Url;

pub struct Remote {
    pub account: Account,
    secret: Credentials,
    http: Client,
    service: Option<Service>,
}
#[derive(Debug)]
pub struct DavObject {
    pub href: String,
    pub etag: Option<String>,
    pub payload: String,
}
impl Remote {
    pub fn new(account: Account, secret: Credentials) -> Result<Self> {
        Ok(Self {
            account,
            secret,
            service: None,
            http: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
        })
    }
    /// Associate health failures with the consumer's grant; the daemon validates the installed caller again.
    pub fn for_service(mut self, service: Service) -> Self {
        self.service = Some(service);
        self
    }
    pub(crate) fn record_http_status(&self, status: reqwest::StatusCode) {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(service) = &self.service {
                let _ = crate::Client::connect().and_then(|client| {
                    client.report_auth_failure(&self.account.id, service.clone())
                });
            }
        }
    }
    pub(crate) fn request(&self, method: Method, url: &Url) -> RequestBuilder {
        let request = self
            .http
            .request(method, url.clone())
            .header("Prefer", "outlook.timezone=\"UTC\", IdType=\"ImmutableId\"");
        if self.secret.access_token.is_empty() {
            request.basic_auth(&self.account.identity, Some(&self.secret.password))
        } else {
            request.bearer_auth(&self.secret.access_token)
        }
    }
    pub(crate) fn text(&self, response: reqwest::blocking::Response) -> Result<String> {
        self.record_http_status(response.status());
        anyhow::ensure!(
            response.status().is_success(),
            "Provider request failed ({})",
            response.status().as_u16()
        );
        let mut data = String::new();
        response
            .take(32 * 1024 * 1024 + 1)
            .read_to_string(&mut data)
            .context("Invalid provider response")?;
        anyhow::ensure!(
            data.len() <= 32 * 1024 * 1024,
            "Provider response exceeded limit"
        );
        Ok(data)
    }
    pub fn json(&self, url: &str) -> Result<Value> {
        self.json_if_present(url)?
            .context("Provider item was not found")
    }
    /// Distinguish a confirmed missing item from network/permission failures before reconciling deletions.
    pub fn json_if_present(&self, url: &str) -> Result<Option<Value>> {
        let uri = Url::parse(url)?;
        let host = if self.account.provider == "google" {
            "www.googleapis.com"
        } else if self.account.provider == "microsoft_personal" {
            "graph.microsoft.com"
        } else {
            anyhow::bail!("Provider has no JSON adapter")
        };
        anyhow::ensure!(
            uri.scheme() == "https"
                && (uri.host_str() == Some(host)
                    || (self.account.provider == "google"
                        && matches!(
                            uri.host_str(),
                            Some(
                                "people.googleapis.com"
                                    | "tasks.googleapis.com"
                                    | "gmail.googleapis.com"
                            )
                        )))
                && uri.port_or_known_default() == Some(443)
                && uri.username().is_empty()
                && uri.password().is_none(),
            "Untrusted API endpoint"
        );
        let response = self
            .request(Method::GET, &uri)
            .send()
            .context("Provider unavailable")?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&self.text(response)?)?))
    }
    pub fn list(&self, service: Service) -> Result<Vec<Value>> {
        anyhow::ensure!(
            self.account.enabled.contains(&service),
            "Service is disabled"
        );
        let start = match (self.account.provider.as_str(), service) {
            ("google", Service::Mail) => "https://gmail.googleapis.com/gmail/v1/users/me/labels",
            ("google", Service::Calendar) => {
                "https://www.googleapis.com/calendar/v3/users/me/calendarList"
            }
            ("google", Service::Contacts) => {
                "https://people.googleapis.com/v1/people/me/connections?personFields=names,emailAddresses,phoneNumbers,addresses,organizations,birthdays,biographies,metadata,urls&sources=READ_SOURCE_TYPE_CONTACT"
            }
            ("google", Service::Files) => "https://www.googleapis.com/drive/v3/files",
            ("google", Service::Tasks) => "https://tasks.googleapis.com/tasks/v1/users/@me/lists",
            ("microsoft_personal", Service::Mail) => {
                "https://graph.microsoft.com/v1.0/me/mailFolders"
            }
            ("microsoft_personal", Service::Calendar) => {
                "https://graph.microsoft.com/v1.0/me/calendars"
            }
            ("microsoft_personal", Service::Contacts) => {
                "https://graph.microsoft.com/v1.0/me/contacts"
            }
            ("microsoft_personal", Service::Files) => {
                "https://graph.microsoft.com/v1.0/me/drive/root/children"
            }
            ("microsoft_personal", Service::Tasks) => {
                "https://graph.microsoft.com/v1.0/me/todo/lists"
            }
            _ => anyhow::bail!("Use DAV discovery for this provider"),
        };
        let mut next = start.to_string();
        let mut result = Vec::new();
        for _ in 0..1000 {
            let page = self.json(&next)?;
            for key in ["items", "connections", "files", "labels", "value"] {
                if let Some(items) = page.get(key).and_then(Value::as_array) {
                    result.extend(items.iter().cloned());
                }
            }
            if let Some(link) = page.get("@odata.nextLink").and_then(Value::as_str) {
                next = link.into();
            } else if let Some(token) = page.get("nextPageToken").and_then(Value::as_str) {
                let mut uri = Url::parse(start)?;
                uri.query_pairs_mut().append_pair("pageToken", token);
                next = uri.into();
            } else {
                return Ok(result);
            }
        }
        anyhow::bail!("Provider pagination exceeded limit")
    }
    pub fn dav_list(&self, endpoint: &str, contacts: bool) -> Result<Vec<DavObject>> {
        let uri = Url::parse(endpoint)?;
        anyhow::ensure!(
            uri.scheme() == "https" && uri.username().is_empty() && uri.password().is_none(),
            "Secure DAV endpoint required"
        );
        let root = if contacts {
            "addressbook-query"
        } else {
            "calendar-query"
        };
        let namespace = if contacts {
            "urn:ietf:params:xml:ns:carddav"
        } else {
            "urn:ietf:params:xml:ns:caldav"
        };
        let property = if contacts {
            "address-data"
        } else {
            "calendar-data"
        };
        let filter = if contacts {
            String::new()
        } else {
            "<c:filter><c:comp-filter name=\"VCALENDAR\"/></c:filter>".into()
        };
        let body = format!(
            "<c:{root} xmlns:d=\"DAV:\" xmlns:c=\"{namespace}\"><d:prop><d:getetag/><c:{property}/></d:prop>{filter}</c:{root}>"
        );
        let response = self
            .request(Method::from_bytes(b"REPORT")?, &uri)
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .context("DAV provider unavailable")?;
        let text = self.text(response)?;
        let xml = roxmltree::Document::parse(&text).context("Invalid DAV response")?;
        let mut objects = Vec::new();
        for response in xml
            .descendants()
            .filter(|n| n.has_tag_name(("DAV:", "response")))
        {
            let Some(href) = response
                .children()
                .find(|n| n.has_tag_name(("DAV:", "href")))
                .and_then(|n| n.text())
            else {
                continue;
            };
            let target = uri.join(href)?;
            anyhow::ensure!(
                target.origin() == uri.origin() && target.path().starts_with(uri.path()),
                "DAV returned a cross-origin resource"
            );
            let payload = response
                .descendants()
                .find(|n| n.has_tag_name((namespace, property)))
                .and_then(|n| n.text());
            anyhow::ensure!(
                payload.is_some(),
                "DAV returned an incomplete report; cached data was retained"
            );
            if let Some(payload) = payload {
                objects.push(DavObject {
                    href: target.into(),
                    etag: response
                        .descendants()
                        .find(|n| n.has_tag_name(("DAV:", "getetag")))
                        .and_then(|n| n.text())
                        .map(str::to_owned),
                    payload: payload.into(),
                });
            }
        }
        Ok(objects)
    }
    pub fn dav_put(
        &self,
        endpoint: &str,
        href: &str,
        payload: &str,
        etag: Option<&str>,
    ) -> Result<String> {
        let base = Url::parse(endpoint)?;
        let target = base.join(href)?;
        anyhow::ensure!(
            base.scheme() == "https"
                && target.origin() == base.origin()
                && target.path().starts_with(base.path()),
            "Untrusted DAV write endpoint"
        );
        let request = self.request(Method::PUT, &target).header(
            "Content-Type",
            if payload
                .trim_start()
                .get(..11)
                .is_some_and(|s| s.eq_ignore_ascii_case("BEGIN:VCARD"))
            {
                "text/vcard; charset=utf-8"
            } else {
                "text/calendar; charset=utf-8"
            },
        );
        let request = if let Some(etag) = etag {
            request.header("If-Match", etag)
        } else {
            request.header("If-None-Match", "*")
        };
        let response = request
            .body(payload.to_owned())
            .send()
            .context("DAV write failed")?;
        self.record_http_status(response.status());
        anyhow::ensure!(
            response.status().is_success(),
            "DAV write refused or conflicted ({})",
            response.status().as_u16()
        );
        Ok(response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into())
    }
}
#[derive(Debug)]
pub struct DavCollection {
    pub components: Vec<String>,
    pub url: String,
    pub name: String,
}
impl Remote {
    pub fn dav_collections(&self, endpoint: &str, contacts: bool) -> Result<Vec<DavCollection>> {
        let origin = Url::parse(endpoint)?;
        let mut current = origin.clone();
        for _ in 0..5 {
            let body = r#"<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="urn:ietf:params:xml:ns:carddav"><d:prop><d:resourcetype/><d:displayname/><d:current-user-principal/><c:calendar-home-set/><c:supported-calendar-component-set/><a:addressbook-home-set/></d:prop></d:propfind>"#;
            let response = self
                .request(Method::from_bytes(b"PROPFIND")?, &current)
                .header("Depth", "1")
                .header("Content-Type", "application/xml")
                .body(body)
                .send()
                .context("DAV discovery failed")?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get("location")
                    .context("DAV redirect missing location")?
                    .to_str()?;
                let next = current.join(location)?;
                anyhow::ensure!(
                    next.origin() == origin.origin(),
                    "Cross-origin DAV redirect refused"
                );
                current = next;
                continue;
            }
            let text = self.text(response)?;
            let xml = roxmltree::Document::parse(&text)?;
            let namespace = if contacts {
                "urn:ietf:params:xml:ns:carddav"
            } else {
                "urn:ietf:params:xml:ns:caldav"
            };
            let resource = if contacts { "addressbook" } else { "calendar" };
            let home = if contacts {
                "addressbook-home-set"
            } else {
                "calendar-home-set"
            };
            let mut result = Vec::new();
            for response in xml
                .descendants()
                .filter(|n| n.has_tag_name(("DAV:", "response")))
            {
                if !response
                    .descendants()
                    .any(|n| n.has_tag_name((namespace, resource)))
                {
                    continue;
                }
                let Some(href) = response
                    .children()
                    .find(|n| n.has_tag_name(("DAV:", "href")))
                    .and_then(|n| n.text())
                else {
                    continue;
                };
                let url = current.join(href)?;
                anyhow::ensure!(url.origin() == origin.origin(), "Untrusted DAV collection");
                let name = response
                    .descendants()
                    .find(|n| n.has_tag_name(("DAV:", "displayname")))
                    .and_then(|n| n.text())
                    .unwrap_or("Online collection")
                    .to_owned();
                let components = response
                    .descendants()
                    .filter(|n| n.has_tag_name(("urn:ietf:params:xml:ns:caldav", "comp")))
                    .filter_map(|n| n.attribute("name").map(str::to_owned))
                    .collect();
                result.push(DavCollection {
                    components,
                    url: url.into(),
                    name,
                });
            }
            if !result.is_empty() {
                return Ok(result);
            }
            let link = xml
                .descendants()
                .find(|n| n.has_tag_name((namespace, home)))
                .or_else(|| {
                    xml.descendants()
                        .find(|n| n.has_tag_name(("DAV:", "current-user-principal")))
                })
                .and_then(|n| n.descendants().find(|n| n.has_tag_name(("DAV:", "href"))))
                .and_then(|n| n.text())
                .context("No DAV collections discovered; enter a collection URL")?;
            let next = current.join(link)?;
            anyhow::ensure!(
                next.origin() == origin.origin(),
                "Untrusted DAV discovery target"
            );
            anyhow::ensure!(next != current, "No accessible DAV collections");
            current = next;
        }
        anyhow::bail!("DAV discovery exceeded limit")
    }
    pub fn dav_delete(&self, endpoint: &str, href: &str, etag: &str) -> Result<()> {
        let base = Url::parse(endpoint)?;
        let target = base.join(href)?;
        anyhow::ensure!(
            base.scheme() == "https"
                && target.origin() == base.origin()
                && target.path().starts_with(base.path()),
            "Untrusted DAV endpoint"
        );
        anyhow::ensure!(!etag.is_empty(), "Deletion requires a known revision");
        let response = self
            .request(Method::DELETE, &target)
            .header("If-Match", etag)
            .send()
            .context("DAV deletion failed")?;
        self.record_http_status(response.status());
        anyhow::ensure!(
            response.status().is_success() || response.status().as_u16() == 404,
            "DAV delete refused or conflicted"
        );
        Ok(())
    }
}
impl Remote {
    pub fn json_write(
        &self,
        endpoint: &str,
        method: Method,
        body: &Value,
        etag: Option<&str>,
    ) -> Result<Value> {
        anyhow::ensure!(
            matches!(method, Method::POST | Method::PATCH | Method::DELETE),
            "Unsupported API operation"
        );
        let uri = Url::parse(endpoint)?;
        let host = match self.account.provider.as_str() {
            "google" => "www.googleapis.com",
            "microsoft_personal" => "graph.microsoft.com",
            _ => anyhow::bail!("Provider has no JSON write adapter"),
        };
        anyhow::ensure!(
            uri.scheme() == "https"
                && (uri.host_str() == Some(host)
                    || (self.account.provider == "google"
                        && matches!(
                            uri.host_str(),
                            Some(
                                "people.googleapis.com"
                                    | "tasks.googleapis.com"
                                    | "gmail.googleapis.com"
                            )
                        )))
                && uri.port_or_known_default() == Some(443)
                && uri.username().is_empty()
                && uri.password().is_none(),
            "Untrusted API write endpoint"
        );
        let deleting = method == Method::DELETE;
        let mut request = self.request(method, &uri).json(body);
        if let Some(etag) = etag {
            request = request.header("If-Match", etag);
        }
        let response = request
            .send()
            .context("Provider write could not be completed")?;
        if response.status() == reqwest::StatusCode::NO_CONTENT
            || (deleting && response.status() == reqwest::StatusCode::NOT_FOUND)
        {
            return Ok(Value::Null);
        }
        if !response.status().is_success() {
            self.record_http_status(response.status());
            return Err(ApiWriteError {
                status: response.status().as_u16(),
            }
            .into());
        }
        let text = self.text(response)?;
        if text.trim().is_empty() {
            Ok(Value::Null)
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }
}
#[derive(Debug)]
pub struct ApiWriteError {
    pub status: u16,
}
impl std::fmt::Display for ApiWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Provider rejected the operation (HTTP {})", self.status)
    }
}
impl std::error::Error for ApiWriteError {}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use std::{
        collections::BTreeSet,
        io::BufRead,
        process::{Command, Stdio},
    };
    struct Fixture {
        dir: std::path::PathBuf,
        child: Option<std::process::Child>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
    #[test]
    fn tls_dav_contact_writes_and_scheduling_use_verified_transport() {
        let dir = std::env::temp_dir().join(format!("nyx-dav-tls-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let mut fixture = Fixture {
            dir: dir.clone(),
            child: None,
        };
        let cert = dir.join("cert.pem");
        let key = dir.join("key.pem");
        let ca = dir.join("ca.pem");
        let ca_key = dir.join("ca.key");
        let csr = dir.join("server.csr");
        let extensions = dir.join("server.ext");
        std::fs::write(&extensions,"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1\n").unwrap();
        let run = |command: &mut Command| {
            assert!(
                command
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=Nyx Test CA",
                "-keyout",
            ])
            .arg(&ca_key)
            .arg("-out")
            .arg(&ca));
        run(Command::new("openssl")
            .args([
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=127.0.0.1",
                "-keyout",
            ])
            .arg(&key)
            .arg("-out")
            .arg(&csr));
        run(Command::new("openssl")
            .args(["x509", "-req", "-days", "1", "-in"])
            .arg(&csr)
            .arg("-CA")
            .arg(&ca)
            .arg("-CAkey")
            .arg(&ca_key)
            .arg("-CAcreateserial")
            .arg("-extfile")
            .arg(&extensions)
            .arg("-out")
            .arg(&cert));
        let mut child = Command::new("python3")
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/dav_server.py"),
            )
            .arg(&cert)
            .arg(&key)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut port = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        let port = port.trim().parse::<u16>().unwrap();
        fixture.child = Some(child);
        let base = format!("https://127.0.0.1:{port}");
        let account = Account {
            id: "fixture".into(),
            provider: "nextcloud".into(),
            identity: "fixture@example.test".into(),
            label: "Fixture".into(),
            enabled: BTreeSet::from([Service::Calendar, Service::Contacts]),
            status: "configured".into(),
            client_id: None,
            endpoints: Default::default(),
            legacy_id: None,
            error: None,
            provider_user_id: None,
        };
        let mut secret = Credentials::empty();
        secret.password = "fixture-password".into();
        let mut remote = Remote::new(account, secret).unwrap();
        remote.http = Client::builder()
            .add_root_certificate(
                reqwest::Certificate::from_pem(&std::fs::read(&ca).unwrap()).unwrap(),
            )
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let payload = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:fixture\r\nFN:Fixture\r\nEND:VCARD\r\n";
        assert_eq!(
            remote
                .dav_put(&format!("{base}/book/"), "new.vcf", payload, None)
                .unwrap(),
            "\"v1\""
        );
        assert_eq!(
            remote
                .dav_put(&format!("{base}/book/"), "new.vcf", payload, Some("\"v1\""))
                .unwrap(),
            "\"v2\""
        );
        assert!(
            remote
                .dav_put(
                    &format!("{base}/book/"),
                    "conflict.vcf",
                    payload,
                    Some("\"v1\"")
                )
                .is_err()
        );
        remote
            .dav_delete(&format!("{base}/book/"), "new.vcf", "\"v2\"")
            .unwrap();
        let availability = remote
            .dav_free_busy(
                &format!("{base}/calendars"),
                "20261030T120000Z",
                "20261030T140000Z",
                "20261030T110000Z",
                &["friend@example.test".into()],
            )
            .unwrap();
        assert_eq!(availability[0].0, "friend@example.test");
        assert!(availability[0].1.contains("BEGIN:VFREEBUSY"));
    }
}
