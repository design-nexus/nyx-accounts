use crate::model::{Account, Credentials};
use anyhow::{Context, Result};
use secret_service::{EncryptionType, SecretService};
use std::{collections::HashMap, os::unix::fs::PermissionsExt, path::Path};

pub struct Store {
    db: rusqlite::Connection,
}
impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        let path = dir.join("accounts.sqlite3");
        let db = rusqlite::Connection::open(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS accounts(id TEXT PRIMARY KEY, metadata TEXT NOT NULL);")?;
        Ok(Self { db })
    }
    pub fn list(&self) -> Result<Vec<Account>> {
        let mut query = self
            .db
            .prepare("SELECT metadata FROM accounts ORDER BY id")?;
        let rows = query.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn get(&self, id: &str) -> Result<Account> {
        let data: String = self
            .db
            .query_row("SELECT metadata FROM accounts WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .context("Account not found")?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn save(&self, account: &Account) -> Result<()> {
        account.validate()?;
        self.db.execute("INSERT INTO accounts VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata", rusqlite::params![account.id, serde_json::to_string(account)?])?;
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.db.execute("DELETE FROM accounts WHERE id=?1", [id])?;
        Ok(())
    }
}
fn attributes(id: &str) -> HashMap<&str, &str> {
    HashMap::from([("application", "nyx-accounts-preview"), ("account", id)])
}
async fn secret_service() -> Result<SecretService<'static>> {
    if let Ok(address) = std::env::var("NYX_ACCOUNTS_SECRET_BUS_ADDRESS") {
        anyhow::ensure!(
            address.starts_with("unix:"),
            "Secret Service requires a local Unix session bus"
        );
        let connection = zbus::connection::Builder::address(address.as_str())?
            .build()
            .await?;
        Ok(SecretService::connect_with_existing(EncryptionType::Dh, connection).await?)
    } else {
        Ok(SecretService::connect(EncryptionType::Dh).await?)
    }
}
pub async fn save_secret(id: &str, value: &Credentials) -> Result<()> {
    let service = secret_service()
        .await
        .context("Secret Service unavailable")?;
    let collection = service.get_default_collection().await?;
    anyhow::ensure!(
        !collection.is_locked().await?,
        "Keyring is locked; unlock it before saving an account"
    );
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(value)?);
    collection
        .create_item(
            "Nyx Online Accounts preview",
            attributes(id),
            &bytes,
            true,
            "application/json",
        )
        .await?;
    Ok(())
}
pub async fn load_secret(id: &str) -> Result<Credentials> {
    let service = secret_service().await?;
    let found = service.search_items(attributes(id)).await?;
    let item = found
        .unlocked
        .into_iter()
        .next()
        .context("Credentials unavailable or keyring locked")?;
    let bytes = zeroize::Zeroizing::new(item.get_secret().await?);
    Ok(serde_json::from_slice(&bytes)?)
}
pub async fn delete_secret(id: &str) -> Result<()> {
    let service = secret_service().await?;
    let found = service.search_items(attributes(id)).await?;
    anyhow::ensure!(
        found.locked.is_empty(),
        "Unlock the keyring before removing this account"
    );
    for item in found.unlocked {
        item.delete().await?;
    }
    Ok(())
}
pub async fn ensure_secret(id: &str) -> Result<()> {
    let service = secret_service().await?;
    let found = service.search_items(attributes(id)).await?;
    anyhow::ensure!(found.locked.is_empty(), "Unlock the keyring before sign-in");
    if found.unlocked.is_empty() {
        save_secret(id, &Credentials::empty()).await?;
    }
    Ok(())
}
