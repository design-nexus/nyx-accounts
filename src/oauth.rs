use crate::{
    Authorization, Provider,
    model::{Account, Credentials, Service},
    store,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
use tokio::io::AsyncReadExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Clone, Debug)]
pub struct Identity {
    pub email: String,
    pub subject: String,
}
fn validate_identity(account: &Account, identity: &Identity) -> Result<()> {
    if let Some(subject) = &account.provider_user_id {
        anyhow::ensure!(
            subject == &identity.subject,
            "Sign in with the account already linked to this profile; add a separate account to use another identity"
        );
    } else {
        anyhow::ensure!(
            account.identity.eq_ignore_ascii_case(&identity.email),
            "Sign in with the email entered for this profile; use its primary email or add a separate account"
        );
    }
    Ok(())
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn provider(account: &Account) -> Result<Provider> {
    match account.provider.as_str() {
        "google" => Ok(Provider::Google),
        "microsoft_personal" => Ok(Provider::MicrosoftPersonal),
        _ => anyhow::bail!("This provider uses an app password"),
    }
}
pub fn scopes(account: &Account) -> Vec<&'static str> {
    let mut scopes = Vec::new();
    if account.provider == "google" {
        scopes.extend(["openid", "email"]);
        for service in &account.enabled {
            scopes.push(match service {
                Service::Mail => "https://mail.google.com/",
                Service::Calendar => "https://www.googleapis.com/auth/calendar",
                Service::Contacts => "https://www.googleapis.com/auth/contacts",
                Service::Files => "https://www.googleapis.com/auth/drive",
                Service::Tasks => "https://www.googleapis.com/auth/tasks",
            });
        }
    } else {
        scopes.extend([
            "openid",
            "offline_access",
            "https://graph.microsoft.com/User.Read",
        ]);
        for service in &account.enabled {
            match service {
                Service::Mail => scopes.extend([
                    "https://graph.microsoft.com/Mail.ReadWrite",
                    "https://graph.microsoft.com/Mail.Send",
                ]),
                Service::Calendar => scopes.push("https://graph.microsoft.com/Calendars.ReadWrite"),
                Service::Contacts => scopes.push("https://graph.microsoft.com/Contacts.ReadWrite"),
                Service::Files => scopes.push("https://graph.microsoft.com/Files.ReadWrite"),
                Service::Tasks => scopes.push("https://graph.microsoft.com/Tasks.ReadWrite"),
            }
        }
    }
    scopes
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    token_type: String,
}
fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
async fn exchange(
    account: &Account,
    fields: Vec<(&str, String)>,
    existing: &mut Credentials,
) -> Result<()> {
    exchange_at(
        account,
        provider(account)?.token_endpoint(),
        fields,
        existing,
    )
    .await
}
async fn exchange_at(
    account: &Account,
    endpoint: &str,
    fields: Vec<(&str, String)>,
    existing: &mut Credentials,
) -> Result<()> {
    let mut fields = fields;
    fields.push((
        "client_id",
        account
            .client_id
            .clone()
            .context("OAuth client ID is required")?,
    ));
    if !existing.client_secret.is_empty() {
        fields.push(("client_secret", existing.client_secret.clone()));
    }
    let response = http()?
        .post(endpoint)
        .form(&fields)
        .send()
        .await
        .context("OAuth server could not be reached")?;
    // Never include token response bodies or reqwest errors containing URLs in UI errors.
    anyhow::ensure!(
        response.status().is_success(),
        "Authorization failed; sign in again or check the OAuth registration"
    );
    let token: TokenResponse = response
        .json()
        .await
        .context("Invalid OAuth token response")?;
    anyhow::ensure!(
        token.token_type.eq_ignore_ascii_case("bearer") && !token.access_token.is_empty(),
        "Unsupported OAuth token response"
    );
    existing.access_token = token.access_token;
    if let Some(refresh) = token.refresh_token {
        existing.refresh_token = refresh;
    }
    existing.expires_at = now().saturating_add(token.expires_in);
    Ok(())
}
/// Listener is bound before returning the URL, eliminating the redirect-port race.
pub async fn begin(
    account: &Account,
) -> Result<(
    String,
    impl std::future::Future<Output = Result<Identity>> + Send + 'static,
)> {
    begin_with_gate(account, std::sync::Arc::new(tokio::sync::Mutex::new(()))).await
}
pub async fn begin_with_gate(
    account: &Account,
    gate: std::sync::Arc<tokio::sync::Mutex<()>>,
) -> Result<(
    String,
    impl std::future::Future<Output = Result<Identity>> + Send + 'static,
)> {
    let provider = provider(account)?;
    let client_id = account
        .client_id
        .clone()
        .context("OAuth client ID is required")?;
    anyhow::ensure!(!client_id.trim().is_empty(), "OAuth client ID is required");
    let port = if provider == Provider::MicrosoftPersonal {
        42813
    } else {
        0
    };
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .context("OAuth loopback port is unavailable")?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let auth = Authorization::default();
    let mut url = url::Url::parse(provider.authorization_endpoint())?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", scopes(account).join(" ").as_str()),
        ("state", auth.state.secret()),
        ("code_challenge", auth.challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    if provider == Provider::Google {
        url.query_pairs_mut()
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
    }
    let account = account.clone();
    let future = async move {
        let callback = async {
            loop {
                let (mut stream, peer) = listener.accept().await?;
                if !peer.ip().is_loopback() {
                    continue;
                }
                let line = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    let mut reader = tokio::io::BufReader::new(&mut stream);
                    let mut line = Vec::new();
                    loop {
                        let available = reader.fill_buf().await?;
                        anyhow::ensure!(!available.is_empty(), "Callback request ended");
                        let count = available
                            .iter()
                            .position(|b| *b == b'\n')
                            .map(|n| n + 1)
                            .unwrap_or(available.len());
                        anyhow::ensure!(
                            line.len() + count <= 8192,
                            "Callback request exceeded limit"
                        );
                        line.extend_from_slice(&available[..count]);
                        reader.consume(count);
                        if line.last() == Some(&b'\n') {
                            break;
                        }
                    }
                    Ok::<_, anyhow::Error>(line)
                })
                .await;
                let callback = match line {
                    Ok(Ok(line)) => std::str::from_utf8(&line)
                        .ok()
                        .and_then(|line| line.strip_prefix("GET "))
                        .and_then(|line| line.split_whitespace().next())
                        .filter(|target| target.starts_with('/'))
                        .and_then(|target| {
                            url::Url::parse(&format!("http://127.0.0.1{target}")).ok()
                        }),
                    _ => None,
                };
                let Some(callback) = callback else {
                    let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    continue;
                };
                let pairs: Vec<_> = callback.query_pairs().collect();
                let state: Vec<_> = pairs.iter().filter(|(k, _)| k == "state").collect();
                if callback.path() != "/callback"
                    || state.len() != 1
                    || state[0].1 != *auth.state.secret()
                {
                    stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                    continue;
                }
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nReturn to Nyx Settings to see the sign-in result.").await?;
                anyhow::ensure!(
                    !pairs.iter().any(|(k, _)| k == "error"),
                    "Authorization was declined"
                );
                let codes: Vec<_> = pairs.iter().filter(|(k, _)| k == "code").collect();
                anyhow::ensure!(
                    codes.len() == 1 && !codes[0].1.is_empty(),
                    "Missing authorization code"
                );
                let _refresh_guard = gate.lock().await;
                let mut credentials = store::load_secret(&account.id).await?;
                exchange(
                    &account,
                    vec![
                        ("grant_type", "authorization_code".into()),
                        ("code", codes[0].1.to_string()),
                        ("redirect_uri", redirect),
                        ("code_verifier", auth.verifier.secret().clone()),
                    ],
                    &mut credentials,
                )
                .await?;
                let profile_url = if account.provider == "google" {
                    "https://openidconnect.googleapis.com/v1/userinfo"
                } else {
                    "https://graph.microsoft.com/v1.0/me?$select=id,mail,userPrincipalName"
                };
                let response = http()?
                    .get(profile_url)
                    .bearer_auth(&credentials.access_token)
                    .send()
                    .await
                    .context("Could not verify account identity")?;
                anyhow::ensure!(
                    response.status().is_success(),
                    "Could not verify account identity"
                );
                let profile: serde_json::Value =
                    response.json().await.context("Invalid account profile")?;
                let identity = if account.provider == "google" {
                    anyhow::ensure!(
                        profile.get("email_verified").and_then(|v| v.as_bool()) == Some(true),
                        "Google email is not verified"
                    );
                    profile.get("email").and_then(|v| v.as_str())
                } else {
                    profile
                        .get("mail")
                        .and_then(|v| v.as_str())
                        .or_else(|| profile.get("userPrincipalName").and_then(|v| v.as_str()))
                }
                .context("Account has no usable email address")?
                .to_owned();
                let subject = profile[if account.provider == "google" {
                    "sub"
                } else {
                    "id"
                }]
                .as_str()
                .filter(|s| !s.is_empty())
                .context("Provider identity identifier is missing")?
                .to_owned();
                let identity = Identity {
                    email: identity,
                    subject,
                };
                validate_identity(&account, &identity)?;
                store::save_secret(&account.id, &credentials).await?;
                return Ok(identity);
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(300), callback)
            .await
            .context("Sign-in timed out")?
    };
    Ok((url.into(), future))
}
pub async fn credential(account: &Account) -> Result<Credentials> {
    credential_with_refresh(account, false).await
}
/// Explicit renewal uses the same exchange/storage path as automatic expiry.
pub async fn refresh(account: &Account) -> Result<()> {
    credential_with_refresh(account, true).await.map(|_| ())
}
async fn credential_with_refresh(account: &Account, force: bool) -> Result<Credentials> {
    let mut credentials = store::load_secret(&account.id).await?;
    if !matches!(account.provider.as_str(), "google" | "microsoft_personal") {
        anyhow::ensure!(
            !credentials.password.is_empty(),
            "An app password is required"
        );
    }
    if matches!(account.provider.as_str(), "google" | "microsoft_personal")
        && renew_if_due_at(account, &mut credentials, force, now(), provider(account)?.token_endpoint()).await?
    {
        store::save_secret(&account.id, &credentials).await?;
    }
    Ok(credentials)
}

// The endpoint and clock are supplied internally so expiry behavior is testable
// against a controlled server; production always uses the provider's fixed URL.
async fn renew_if_due_at(account: &Account, credentials: &mut Credentials, force: bool, current_time: u64, endpoint: &str) -> Result<bool> {
    if !force && credentials.expires_at > current_time.saturating_add(60) { return Ok(false); }
    anyhow::ensure!(!credentials.refresh_token.is_empty(), "Sign in again");
    exchange_at(account, endpoint, vec![("grant_type", "refresh_token".into()),("refresh_token",credentials.refresh_token.clone())], credentials).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn server(body: &'static str, status: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let status = status.to_string();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 16384];
            let n = stream.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]).into_owned();
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
            request
        });
        (endpoint, task)
    }
    fn account() -> Account {
        Account {
            id: "fixture".into(),
            provider: "google".into(),
            identity: "test@example.test".into(),
            label: "Test".into(),
            enabled: Default::default(),
            status: "needs_sign_in".into(),
            client_id: Some("fixture-client".into()),
            endpoints: Default::default(),
            legacy_id: None,
            error: None,
            provider_user_id: None,
        }
    }
    #[tokio::test]
    async fn revocation_posts_token_without_exposing_it_in_errors() {
        for (status,ok) in [("200 OK",true),("400 Bad Request",false)] {
            let (endpoint,request)=server("{}",status).await;
            let result=revoke_token_at(&endpoint,"synthetic-private-token").await;
            assert_eq!(result.is_ok(),ok);
            if let Err(error)=result {assert!(!error.to_string().contains("synthetic-private-token"));}
            let wire=request.await.unwrap();
            assert!(wire.starts_with("POST /token HTTP/1.1"));
            assert!(wire.contains("token=synthetic-private-token"));
        }
    }
    #[tokio::test]
    async fn expired_and_expiring_tokens_renew_without_user_sign_in() {
        for expires in [999,1060] {
            let (endpoint,request)=server(r#"{"access_token":"renewed","expires_in":3600,"token_type":"Bearer"}"#,"200 OK").await;
            let mut c=Credentials::empty();c.refresh_token="retained".into();c.expires_at=expires;
            assert!(renew_if_due_at(&account(),&mut c,false,1000,&endpoint).await.unwrap());
            assert_eq!(c.access_token,"renewed");assert_eq!(c.refresh_token,"retained");
            assert!(request.await.unwrap().contains("grant_type=refresh_token"));
        }
    }
    #[tokio::test]
    async fn unexpired_token_does_not_contact_the_provider() {
        let mut c=Credentials::empty();c.access_token="still-valid".into();c.expires_at=1061;
        assert!(!renew_if_due_at(&account(),&mut c,false,1000,"invalid-no-network").await.unwrap());
        assert_eq!(c.access_token,"still-valid");
    }
    #[tokio::test]
    async fn refresh_rotates_token_and_sets_expiration() {
        let (endpoint, request) = server(r#"{"access_token":"fixture-access","refresh_token":"rotated","expires_in":3600,"token_type":"Bearer"}"#, "200 OK").await;
        let mut secret = Credentials::empty();
        secret.refresh_token = "original".into();
        exchange_at(
            &account(),
            &endpoint,
            vec![
                ("grant_type", "refresh_token".into()),
                ("refresh_token", "original".into()),
            ],
            &mut secret,
        )
        .await
        .unwrap();
        assert_eq!(secret.refresh_token, "rotated");
        assert!(secret.expires_at >= now() + 3500);
        let request = request.await.unwrap();
        assert!(request.contains("grant_type=refresh_token"));
        assert!(request.contains("client_id=fixture-client"));
    }
    #[tokio::test]
    async fn authorization_exchange_sends_verifier_and_preserves_refresh_token() {
        let (endpoint, request) = server(
            r#"{"access_token":"fixture-access","expires_in":60,"token_type":"Bearer"}"#,
            "200 OK",
        )
        .await;
        let mut secret = Credentials::empty();
        secret.refresh_token = "keep-me".into();
        exchange_at(
            &account(),
            &endpoint,
            vec![
                ("grant_type", "authorization_code".into()),
                ("code", "fixture-code".into()),
                ("code_verifier", "fixture-verifier".into()),
            ],
            &mut secret,
        )
        .await
        .unwrap();
        assert_eq!(secret.refresh_token, "keep-me");
        assert!(
            request
                .await
                .unwrap()
                .contains("code_verifier=fixture-verifier")
        );
    }
    #[tokio::test]
    async fn provider_error_body_is_not_exposed() {
        let (endpoint, request) = server(
            r#"{"error":"invalid_grant","secret":"must-not-leak"}"#,
            "400 Bad Request",
        )
        .await;
        let error = exchange_at(&account(), &endpoint, vec![], &mut Credentials::empty())
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("must-not-leak"));
        request.await.unwrap();
    }
}

/// Microsoft personal accounts do not support Graph revokeSignInSessions.
/// False means the user must revoke access in their provider account settings.
pub async fn revoke(account: &Account) -> Result<bool> {
    if account.provider != "google" {
        return Ok(false);
    }
    let credentials = store::load_secret(&account.id).await?;
    let token = if credentials.refresh_token.is_empty() {
        &credentials.access_token
    } else {
        &credentials.refresh_token
    };
    if token.is_empty() {
        return Ok(false);
    }
    revoke_token_at("https://oauth2.googleapis.com/revoke", token).await?;
    Ok(true)
}
async fn revoke_token_at(endpoint: &str, token: &str) -> Result<()> {
    let response = http()?
        .post(endpoint)
        .form(&[("token", token)])
        .send()
        .await
        .context("Revocation could not reach Google")?;
    anyhow::ensure!(
        response.status().is_success(),
        "Google revocation failed; revoke access in Google account settings"
    );
    Ok(())
}
#[cfg(test)]
mod callback_tests {
    use super::*;
    #[tokio::test]
    async fn invalid_state_is_rejected_then_denial_completes_once() {
        let account = Account {
            id: "fixture".into(),
            provider: "google".into(),
            identity: "fixture@example.test".into(),
            label: "Test".into(),
            enabled: Default::default(),
            status: "needs_sign_in".into(),
            client_id: Some("fixture-client".into()),
            endpoints: Default::default(),
            legacy_id: None,
            error: None,
            provider_user_id: None,
        };
        let (url, future) = begin(&account).await.unwrap();
        let auth = url::Url::parse(&url).unwrap();
        let params: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
        assert_eq!(params["code_challenge_method"], "S256");
        let redirect = url::Url::parse(&params["redirect_uri"]).unwrap();
        let address = format!("127.0.0.1:{}", redirect.port().unwrap());
        let task = tokio::spawn(future);
        let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();
        stream
            .write_all(b"GET /callback?state=wrong&code=unused HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).await.unwrap();
        assert!(reply.starts_with("HTTP/1.1 400"));
        let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();
        stream.write_all(b"GET /call").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        stream
            .write_all(
                format!(
                    "back?state={}&error=access_denied HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    params["state"]
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert_eq!(error.to_string(), "Authorization was declined");
        assert!(tokio::net::TcpStream::connect(&address).await.is_err());
    }
}
#[cfg(test)]
mod identity_tests {
    use super::*;
    fn account() -> Account {
        serde_json::from_value(serde_json::json!({"id":"fixture","provider":"google","identity":"person@example.test","label":"Fixture","enabled":[],"status":"needs_sign_in"})).unwrap()
    }
    #[test]
    fn new_profile_cannot_silently_switch_to_another_email() {
        assert!(
            validate_identity(
                &account(),
                &Identity {
                    email: "other@example.test".into(),
                    subject: "other".into()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn established_profile_is_bound_to_subject_not_alias() {
        let mut account = account();
        account.provider_user_id = Some("stable-person".into());
        assert!(
            validate_identity(
                &account,
                &Identity {
                    email: "new-alias@example.test".into(),
                    subject: "stable-person".into()
                }
            )
            .is_ok()
        );
        assert!(
            validate_identity(
                &account,
                &Identity {
                    email: account.identity.clone(),
                    subject: "different-person".into()
                }
            )
            .is_err()
        );
    }
}
