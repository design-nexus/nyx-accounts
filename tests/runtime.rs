#[test]
fn public_client_is_usable_from_application_runtime() {
    if std::env::var_os("NYX_ACCOUNTS_RUNTIME_CHILD").is_none() {
        let status = std::process::Command::new("dbus-run-session")
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "public_client_is_usable_from_application_runtime",
                "--nocapture",
            ])
            .env("NYX_ACCOUNTS_RUNTIME_CHILD", "1")
            .status()
            .expect("private test bus");
        assert!(status.success());
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let client = nyx_accounts_service::Client::connect().expect("private session bus");
            drop(client);
            tokio::task::spawn_blocking(|| {
                let client =
                    nyx_accounts_service::Client::connect().expect("blocking application worker");
                drop(client);
            })
            .await
            .unwrap();
        });
}
