use crate::{
    model::{Account, Credentials, Service},
    oauth,
    store::{self, Store},
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
};

pub const BUS: &str = "io.github.design_nexus.nyx.Accounts.Preview";
pub const PATH: &str = "/io/github/design_nexus/nyx/Accounts";
pub const IFACE: &str = "io.github.design_nexus.nyx.Accounts1";
#[derive(Clone)]
pub struct Daemon {
    store: Arc<Mutex<Store>>,
    runtime: PathBuf,
    prefix: PathBuf,
    jobs: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    refresh: Arc<tokio::sync::Mutex<()>>,
}
fn safe_error(_: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(
        "Account operation failed; check account status or unlock the keyring".into(),
    )
}
impl Daemon {
    pub fn new(data: PathBuf, runtime: PathBuf, prefix: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&runtime)?;
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            store: Arc::new(Mutex::new(Store::open(&data)?)),
            runtime,
            prefix,
            jobs: Default::default(),
            refresh: Default::default(),
        })
    }
    async fn manager(
        &self,
        connection: &zbus::Connection,
        header: &zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::AccessDenied("Missing caller".into()))?;
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        let credentials = bus
            .get_connection_credentials(sender.clone().into())
            .await?;
        use std::os::unix::fs::MetadataExt;
        let uid = std::fs::metadata("/proc/self").map_err(safe_error)?.uid();
        if credentials.unix_user_id() != Some(uid) {
            return Err(zbus::fdo::Error::AccessDenied("Caller UID mismatch".into()));
        }
        let pid = credentials
            .process_id()
            .ok_or_else(|| zbus::fdo::Error::AccessDenied("Caller PID unavailable".into()))?;
        let executable = std::fs::read_link(format!("/proc/{pid}/exe")).map_err(safe_error)?;
        if executable.parent() != Some(self.prefix.join("bin").as_path())
            || !matches!(
                executable.file_name().and_then(|s| s.to_str()),
                Some("nyx-settings" | "nyx-accountsctl" | "nyx-pim-migrate")
            )
        {
            return Err(zbus::fdo::Error::AccessDenied(
                "Only installed preview account managers may change accounts".into(),
            ));
        }
        Ok(())
    }
    fn cancel(&self, id: &str) {
        if let Some(job) = self.jobs.lock().unwrap().remove(id) {
            job.abort();
        }
    }
    fn get(&self, id: &str) -> Result<Account> {
        self.store.lock().unwrap().get(id)
    }
    fn save(&self, account: &Account) -> Result<()> {
        self.store.lock().unwrap().save(account)
    }
    fn authorized(
        &self,
        stream: &tokio::net::UnixStream,
        manager: bool,
    ) -> Result<Option<Service>> {
        let cred = stream.peer_cred()?;
        let own = std::fs::metadata("/proc/self")?;
        use std::os::unix::fs::MetadataExt;
        anyhow::ensure!(cred.uid() == own.uid(), "Caller UID mismatch");
        let pid = cred.pid().context("Caller PID unavailable")?;
        let executable = std::fs::read_link(format!("/proc/{pid}/exe"))?;
        let bin = self.prefix.join("bin");
        let name = executable
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        anyhow::ensure!(
            executable.parent() == Some(bin.as_path()),
            "Caller is not an installed preview consumer"
        );
        if manager {
            anyhow::ensure!(
                matches!(name, "nyx-settings" | "nyx-accountsctl"),
                "Caller cannot manage credentials"
            );
            return Ok(None);
        }
        Ok(Some(match name {
            "nyx-maild" => Service::Mail,
            "nyx-calendar" | "nyx-calendard" => Service::Calendar,
            "nyx-contacts" => Service::Contacts,
            "nyx-reminders" | "nyx-remindersd" => Service::Tasks,
            "nyx-cloudd" => Service::Files,
            _ => anyhow::bail!("Caller cannot request credentials"),
        }))
    }
    async fn session(&self, id: String, manager: bool) -> Result<String> {
        self.get(&id)?;
        let path = self.runtime.join(format!("{}.sock", uuid::Uuid::new_v4()));
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let daemon = self.clone();
        let path_string = path.to_string_lossy().into_owned();
        tokio::spawn(async move {
            let work = async {
                let (stream, _) = listener.accept().await?;
                let service = daemon.authorized(&stream, manager)?;
                let mut stream = BufReader::new(stream);
                let mut line = Vec::new();
                // Cap input before allocation can grow unbounded.
                loop {
                    let available = stream.fill_buf().await?;
                    anyhow::ensure!(!available.is_empty(), "Session ended");
                    let count = available
                        .iter()
                        .position(|b| *b == b'\n')
                        .map(|n| n + 1)
                        .unwrap_or(available.len());
                    anyhow::ensure!(line.len() + count <= 65536, "Session request too large");
                    line.extend_from_slice(&available[..count]);
                    stream.consume(count);
                    if line.last() == Some(&b'\n') {
                        break;
                    }
                }
                let line = zeroize::Zeroizing::new(line);
                let account = daemon.get(&id)?;
                if manager {
                    let credentials: Credentials = serde_json::from_slice(&line)?;
                    anyhow::ensure!(
                        credentials.access_token.is_empty()
                            && credentials.refresh_token.is_empty()
                            && credentials.expires_at == 0,
                        "Tokens must come from browser authorization, not manager input"
                    );
                    let _guard = daemon.refresh.lock().await;
                    let current = daemon.get(&id)?;
                    anyhow::ensure!(current.status != "removing", "Account is being removed");
                    store::save_secret(&id, &credentials).await?;
                    let mut account = daemon.get(&id)?;
                    account.status = if credentials.password.is_empty() {
                        "needs_sign_in"
                    } else {
                        "configured"
                    }
                    .into();
                    daemon.save(&account)?;
                    stream.get_mut().write_all(b"{\"ok\":true}\n").await?;
                } else {
                    let request: serde_json::Value = serde_json::from_slice(&line)?;
                    let service = service.context("Missing grant")?;
                    anyhow::ensure!(account.enabled.contains(&service), "Service is disabled");
                    let expected = serde_json::to_value(&service)?;
                    anyhow::ensure!(
                        request.get("service") == Some(&expected),
                        "Service grant mismatch"
                    );
                    if request.get("failure").and_then(|v| v.as_bool()) == Some(true) {
                        let _guard = daemon.refresh.lock().await;
                        let mut current = daemon.get(&id)?;
                        if current.status != "authorizing"
                            && current.status != "removing"
                            && current.enabled.contains(&service)
                        {
                            current.status = "needs_sign_in".into();
                            current.error=Some("The provider rejected sign-in. Check the account’s app password or authorize it again.".into());
                            daemon.save(&current)?;
                        }
                        stream.get_mut().write_all(b"{\"ok\":true}\n").await?;
                        return Ok(());
                    }
                    let _refresh = daemon.refresh.lock().await;
                    let account = daemon.get(&id)?;
                    anyhow::ensure!(account.enabled.contains(&service), "Service is disabled");
                    anyhow::ensure!(
                        account.status != "removing"
                            && account.status != "needs_sign_in"
                            && account.status != "authorizing",
                        "Account needs attention"
                    );
                    let credentials = match oauth::credential(&account).await {
                        Ok(credentials) => credentials,
                        Err(error) => {
                            let mut failed = account.clone();
                            failed.status = "needs_sign_in".into();
                            failed.error=Some("Authorization is unavailable. Unlock the keyring, check your network, or sign in again.".into());
                            daemon.save(&failed)?;
                            return Err(error);
                        }
                    };
                    let current = daemon.get(&id)?;
                    anyhow::ensure!(
                        current.enabled.contains(&service) && current.status != "removing",
                        "Account service was disabled"
                    );
                    // Only the usable access credential crosses this boundary.
                    let response = serde_json::json!({"access_token": credentials.access_token, "password": credentials.password, "expires_at": credentials.expires_at});
                    let mut bytes = zeroize::Zeroizing::new(serde_json::to_vec(&response)?);
                    bytes.push(b'\n');
                    stream.get_mut().write_all(&bytes).await?;
                }
                Ok::<_, anyhow::Error>(())
            };
            let _ = tokio::time::timeout(std::time::Duration::from_secs(30), work).await;
            let _ = tokio::fs::remove_file(path).await;
        });
        Ok(path_string)
    }
}
#[zbus::interface(name = "io.github.design_nexus.nyx.Accounts1")]
impl Daemon {
    async fn list_accounts(&self) -> zbus::fdo::Result<String> {
        let accounts = self.store.lock().unwrap().list().map_err(safe_error)?;
        serde_json::to_string(&accounts).map_err(safe_error)
    }
    async fn register(
        &self,
        metadata: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        self.manager(connection, &header).await?;
        let mut account: Account = serde_json::from_str(metadata).map_err(safe_error)?;
        crate::providers::apply_presets(&mut account, None).map_err(safe_error)?;
        account.id = uuid::Uuid::new_v4().to_string();
        account.status = "needs_sign_in".into();
        account.provider_user_id = None;
        account.error = None;
        self.save(&account).map_err(safe_error)?;
        Ok(account.id)
    }
    async fn update_configuration(
        &self,
        metadata: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.manager(connection, &header).await?;
        let _guard = self.refresh.lock().await;
        let mut account: Account = serde_json::from_str(metadata).map_err(safe_error)?;
        let previous = self.get(&account.id).map_err(safe_error)?;
        let client_changed = account.client_id != previous.client_id;
        let auth_changed = client_changed || account.enabled != previous.enabled;
        if auth_changed {
            self.cancel(&account.id);
        }
        account.provider_user_id = previous.provider_user_id.clone();
        account.identity = previous.identity.clone();
        account.legacy_id = previous.legacy_id.clone();
        if client_changed {
            store::save_secret(&account.id, &Credentials::empty())
                .await
                .map_err(safe_error)?;
        }
        if account.provider != previous.provider {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Provider cannot be changed".into(),
            ));
        }
        account.error = previous.error.clone();
        account.status = if client_changed
            || (auth_changed && previous.status == "authorizing")
            || !account.enabled.is_subset(&previous.enabled)
        {
            "needs_sign_in".into()
        } else {
            previous.status
        };
        self.save(&account).map_err(safe_error)
    }
    async fn check_account(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        self.manager(connection, &header).await?;
        let _gate = self.refresh.lock().await;
        let mut account = self.get(id).map_err(safe_error)?;
        if matches!(account.status.as_str(), "authorizing" | "removing") {
            return Ok(account.status);
        }
        let status = match oauth::credential(&account).await {
            Ok(credentials) => {
                if credentials.access_token.is_empty() {
                    "configured"
                } else {
                    "connected"
                }
            }
            Err(_) => "needs_sign_in",
        };
        account.status = status.into();
        account.error = if status == "needs_sign_in" {
            Some("Credentials are unavailable. Unlock Secret Service, check the network, or sign in again.".into())
        } else {
            None
        };
        self.save(&account).map_err(safe_error)?;
        Ok(status.into())
    }
    /// Manager-only renewal; no credential material is returned over D-Bus.
    async fn refresh_account(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<bool> {
        self.manager(connection, &header).await?;
        let _gate = self.refresh.lock().await;
        let mut account = self.get(id).map_err(safe_error)?;
        if !matches!(account.provider.as_str(), "google" | "microsoft_personal") {
            return Err(zbus::fdo::Error::InvalidArgs("This provider uses an app password".into()));
        }
        if account.status != "connected" {
            return Err(zbus::fdo::Error::Failed("Sign in before renewing authorization".into()));
        }
        oauth::refresh(&account).await.map_err(safe_error)?;
        account.status = "connected".into();
        account.error = None;
        self.save(&account).map_err(safe_error)?;
        Ok(true)
    }
    async fn revoke_authorization(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<bool> {
        self.manager(connection, &header).await?;
        self.cancel(id);
        let _guard = self.refresh.lock().await;
        let mut account = self.get(id).map_err(safe_error)?;
        account.status = "needs_sign_in".into();
        self.save(&account).map_err(safe_error)?;
        let revoked = oauth::revoke(&account).await.map_err(safe_error)?;
        store::delete_secret(id).await.map_err(safe_error)?;
        Ok(revoked)
    }
    async fn set_services(
        &self,
        id: &str,
        enabled: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.manager(connection, &header).await?;
        let _guard = self.refresh.lock().await;
        let mut account = self.get(id).map_err(safe_error)?;
        let services = serde_json::from_str(enabled).map_err(safe_error)?;
        let changed = account.enabled != services;
        if changed {
            self.cancel(id);
        }
        if matches!(account.provider.as_str(), "google" | "microsoft_personal")
            && (!std::collections::BTreeSet::is_subset(&services, &account.enabled)
                || (changed && account.status == "authorizing"))
        {
            account.status = "needs_sign_in".into();
        }
        account.enabled = services;
        self.save(&account).map_err(safe_error)
    }
    async fn begin_secret_update(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        self.manager(connection, &header).await?;
        self.session(id.into(), true).await.map_err(safe_error)
    }
    async fn begin_access(&self, id: &str) -> zbus::fdo::Result<String> {
        self.session(id.into(), false).await.map_err(safe_error)
    }
    async fn begin_authorization(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        self.manager(connection, &header).await?;
        let _guard = self.refresh.lock().await;
        self.cancel(id);
        let mut account = self.get(id).map_err(safe_error)?;
        store::ensure_secret(id).await.map_err(safe_error)?;
        let (url, future) = oauth::begin_with_gate(&account, self.refresh.clone())
            .await
            .map_err(safe_error)?;
        account.status = "authorizing".into();
        account.error = None;
        self.save(&account).map_err(safe_error)?;
        let expected_client = account.client_id.clone();
        let expected_services = account.enabled.clone();
        let daemon = self.clone();
        let job_id = id.to_owned();
        let job = tokio::spawn(async move {
            let result = future.await;
            let _guard = daemon.refresh.lock().await;
            if let Ok(mut account) = daemon.get(&job_id) {
                account.status = if result.is_ok() {
                    "connected"
                } else {
                    "needs_sign_in"
                }
                .into();
                if let Ok(identity) = result {
                    if account.client_id != expected_client || account.enabled != expected_services
                    {
                        account.status = "needs_sign_in".into();
                        account.error = Some(
                            "Account configuration changed during sign-in. Authorize it again."
                                .into(),
                        );
                    } else {
                        account.identity = identity.email;
                        account.provider_user_id = Some(identity.subject);
                        account.error = None;
                    }
                } else if let Err(error) = result {
                    let message = error.to_string();
                    account.error=Some(if message.contains("account already linked"){
                        "Use the account already linked to this profile. Add a separate account to connect another identity."
                    }else if message.contains("email entered"){
                        "Use this account’s primary email address and sign in with that account. Existing cached data was not reassigned."
                    }else if message.contains("declined"){
                        "Authorization was declined. Sign in again when ready."
                    }else if message.contains("timed out"){
                        "Sign-in timed out. Try again."
                    }else{"Sign-in did not complete. Check your client registration, consent/test-user settings, keyring, and network connection."}.into());
                }
                let _ = daemon.save(&account);
            }
        });
        self.jobs.lock().unwrap().insert(id.into(), job);
        Ok(url)
    }
    async fn cancel_authorization(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.manager(connection, &header).await?;
        let _guard = self.refresh.lock().await;
        if let Some(job) = self.jobs.lock().unwrap().remove(id) {
            job.abort();
        }
        if let Ok(mut account) = self.get(id) {
            if account.status == "authorizing" {
                account.status = "needs_sign_in".into();
                self.save(&account).map_err(safe_error)?;
            }
        }
        Ok(())
    }
    async fn remove_account(
        &self,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.manager(connection, &header).await?;
        self.cancel(id);
        let _guard = self.refresh.lock().await;
        let mut account = self.get(id).map_err(safe_error)?;
        account.enabled.clear();
        account.status = "removing".into();
        self.save(&account).map_err(safe_error)?;
        store::delete_secret(id).await.map_err(safe_error)?;
        self.store.lock().unwrap().remove(id).map_err(safe_error)
    }
}
