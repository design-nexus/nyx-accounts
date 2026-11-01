//! Explicit opt-in check for an independently supplied isolated Secret Service fixture.
#[tokio::test]
#[ignore = "requires a separately supplied temporary Secret Service fixture; never uses the desktop keyring"]
async fn desktop_keyring_roundtrip_and_rotation() {
    use nyx_accounts_service::{model::Credentials, store};
    let address=std::env::var("NYX_ACCOUNTS_SECRET_BUS_ADDRESS").expect("An isolated fixture bus must be supplied explicitly");
    let temporary=std::env::temp_dir();
    let socket=address.strip_prefix("unix:path=").expect("Fixture requires a local socket path");
    assert!(std::path::Path::new(socket).starts_with(&temporary),"Desktop keyring tests are prohibited");
    assert!(std::env::var_os("HOME").map(std::path::PathBuf::from).is_some_and(|h|h.starts_with(&temporary)),"Fixture HOME must be isolated; never use the desktop credential store");
    let id = format!("fixture-{}", uuid::Uuid::new_v4());
    let result: anyhow::Result<()> = async {
        store::ensure_secret(&id).await?;
        let mut fixture = Credentials::empty();
        fixture.refresh_token = "synthetic-first".into();
        fixture.password = "synthetic-password".into();
        store::save_secret(&id, &fixture).await?;
        let loaded = store::load_secret(&id).await?;
        anyhow::ensure!(
            loaded.refresh_token == fixture.refresh_token && loaded.password == fixture.password,
            "Fixture round trip failed"
        );
        fixture.refresh_token = "synthetic-rotated".into();
        store::save_secret(&id, &fixture).await?;
        let rotated = store::load_secret(&id).await?;
        anyhow::ensure!(
            rotated.refresh_token == fixture.refresh_token,
            "Fixture rotation failed"
        );
        Ok(())
    }
    .await;
    let cleanup = store::delete_secret(&id).await;
    assert!(
        cleanup.is_ok(),
        "Could not remove disposable keyring fixture {id}"
    );
    assert!(
        result.is_ok(),
        "Secret Service fixture failed: {}",
        result.err().unwrap()
    );
    assert!(
        store::load_secret(&id).await.is_err(),
        "Deleted fixture remained readable"
    );
}
