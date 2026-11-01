use anyhow::{Context, Result};
use nyx_accounts_service::daemon::{BUS, Daemon, PATH};
use std::path::PathBuf;
#[tokio::main]
async fn main() -> Result<()> {
    let home = PathBuf::from(std::env::var_os("HOME").context("HOME unavailable")?);
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("nyx-accounts-preview");
    let runtime =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR unavailable")?)
            .join("nyx-accounts-preview");
    let prefix = std::env::current_exe()?
        .parent()
        .context("Missing executable directory")?
        .parent()
        .context("Missing installation prefix")?
        .to_owned();
    let daemon = Daemon::new(data, runtime, prefix)?;
    let _connection = zbus::connection::Builder::session()?
        .name(BUS)?
        .serve_at(PATH, daemon)?
        .build()
        .await?;
    std::future::pending::<()>().await;
    Ok(())
}
