use anyhow::{Context, Result};
use nyx_accounts_service::{
    client::Client,
    model::{Account, Credentials},
};
use std::io::Read;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let client = Client::connect()?;
    match args.first().map(String::as_str) {
        Some("list") => println!("{}", serde_json::to_string_pretty(&client.list()?)?),
        Some("register") => {
            let mut input = String::new();
            std::io::stdin().take(65536).read_to_string(&mut input)?;
            let account: Account = serde_json::from_str(&input)?;
            println!("{}", client.register(&account)?);
        }
        Some("secret") => {
            let id = args.get(1).context("Account ID required")?;
            let mut input = zeroize::Zeroizing::new(String::new());
            std::io::stdin().take(65536).read_to_string(&mut input)?;
            let secret: Credentials = serde_json::from_str(&input)?;
            client.save_secret(id, &secret)?;
        }
        Some("authorize") => println!(
            "{}",
            client.authorize(args.get(1).context("Account ID required")?)?
        ),
        Some("revoke") => {
            let revoked: bool = client.call(
                "RevokeAuthorization",
                (args.get(1).context("Account ID required")?.as_str(),),
            )?;
            println!(
                "{}",
                if revoked {
                    "Provider authorization revoked"
                } else {
                    "Local credentials removed; revoke provider-side access manually"
                }
            );
        }
        Some("remove") => client.remove(args.get(1).context("Account ID required")?)?,
        _ => anyhow::bail!(
            "Usage: nyx-accountsctl list | register < metadata.json | secret ID < credentials.json | authorize ID | remove ID"
        ),
    }
    Ok(())
}
