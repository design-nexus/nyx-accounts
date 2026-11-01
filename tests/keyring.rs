//! Explicit opt-in check using disposable synthetic values in the existing unlocked Secret Service.
#[tokio::test]
#[ignore = "requires an unlocked desktop Secret Service; creates and removes one unique fixture entry"]
async fn desktop_keyring_roundtrip_and_rotation() {
    use nyx_accounts_service::{model::Credentials, store};
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
