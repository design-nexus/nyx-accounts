//! Public metadata API. Credential payloads use private Unix sessions only.
use crate::{
    daemon::{BUS, IFACE, PATH},
    model::{Account, Credentials},
};
use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

pub struct Client {
    connection: gio::DBusConnection,
    watches: std::cell::RefCell<Vec<glib::SourceId>>,
}
impl Client {
    pub fn connect() -> Result<Self> {
        Ok(Self {
            connection: gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)?,
            watches: Default::default(),
        })
    }
    pub fn call<T: serde::de::DeserializeOwned + zbus::zvariant::Type>(
        &self,
        method: &str,
        args: impl serde::Serialize + zbus::zvariant::DynamicType,
    ) -> Result<T> {
        use glib::variant::ToVariant;
        let args = serde_json::to_value(args)?;
        let args = match args {
            serde_json::Value::Null => Vec::new(),
            serde_json::Value::Array(args) => args,
            _ => anyhow::bail!("D-Bus arguments must be a tuple"),
        };
        let values: Vec<glib::Variant> = args
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(value) => Ok(value.to_variant()),
                serde_json::Value::Bool(value) => Ok(value.to_variant()),
                _ => anyhow::bail!("Unsupported D-Bus argument type"),
            })
            .collect::<Result<_>>()?;
        let parameters = glib::Variant::tuple_from_iter(values);
        let reply = self.connection.call_sync(
            Some(BUS),
            PATH,
            IFACE,
            method,
            Some(&parameters),
            None,
            gio::DBusCallFlags::NONE,
            80000,
            gio::Cancellable::NONE,
        )?;
        let value = if reply.n_children() == 0 {
            serde_json::Value::Null
        } else {
            let value = reply.child_value(0);
            if let Some(value) = value.get::<String>() {
                serde_json::Value::String(value)
            } else if let Some(value) = value.get::<bool>() {
                serde_json::Value::Bool(value)
            } else {
                anyhow::bail!("Unsupported D-Bus reply type")
            }
        };
        Ok(serde_json::from_value(value)?)
    }

    pub fn list(&self) -> Result<Vec<Account>> {
        let text: String = self.call("ListAccounts", ())?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn register(&self, account: &Account) -> Result<String> {
        self.call("Register", (serde_json::to_string(account)?,))
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.call("RemoveAccount", (id,))
    }
    pub fn authorize(&self, id: &str) -> Result<String> {
        self.call("BeginAuthorization", (id,))
    }
    pub fn set_services(
        &self,
        id: &str,
        services: &std::collections::BTreeSet<crate::model::Service>,
    ) -> Result<()> {
        self.call("SetServices", (id, serde_json::to_string(services)?))
    }
    pub fn save_secret(&self, id: &str, secret: &Credentials) -> Result<()> {
        let path: String = self.call("BeginSecretUpdate", (id,))?;
        let mut stream = session(&path)?;
        let mut bytes = zeroize::Zeroizing::new(serde_json::to_vec(secret)?);
        bytes.push(b'\n');
        stream.write_all(&bytes)?;
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply)?;
        anyhow::ensure!(
            reply.contains("\"ok\":true"),
            "Credential save failed; unlock the keyring and retry"
        );
        Ok(())
    }
    pub fn report_auth_failure(&self, id: &str, service: crate::model::Service) -> Result<()> {
        let path: String = self.call("BeginAccess", (id,))?;
        let mut stream = session(&path)?;
        writeln!(
            stream,
            "{}",
            serde_json::json!({"service":service,"failure":true})
        )?;
        let mut reply = String::new();
        BufReader::new(stream).take(4096).read_line(&mut reply)?;
        anyhow::ensure!(
            reply.contains("\"ok\":true"),
            "Could not report account failure"
        );
        Ok(())
    }
    pub fn credential(&self, id: &str, service: crate::model::Service) -> Result<Credentials> {
        let path: String = self.call("BeginAccess", (id,))?;
        let mut stream = session(&path)?;
        writeln!(stream, "{}", serde_json::json!({"service":service}))?;
        let mut reply = zeroize::Zeroizing::new(String::new());
        BufReader::new(stream).take(65536).read_line(&mut reply)?;
        #[derive(serde::Deserialize)]
        struct Access {
            access_token: String,
            password: String,
            expires_at: u64,
        }
        let access: Access = serde_json::from_str(&reply)?;
        Ok(Credentials {
            access_token: access.access_token,
            password: access.password,
            expires_at: access.expires_at,
            refresh_token: String::new(),
            client_secret: String::new(),
        })
    }
}
fn session(path: &str) -> Result<UnixStream> {
    let stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    Ok(stream)
}
use std::io::Read;

impl Client {
    pub fn watch(&self, callback: impl Fn() + 'static) {
        let mut previous = String::new();
        let connection = self.connection.clone();
        let source = glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            if let Ok(reply) = connection.call_sync(
                Some(BUS),
                PATH,
                IFACE,
                "ListAccounts",
                None,
                None,
                gio::DBusCallFlags::NONE,
                8000,
                gio::Cancellable::NONE,
            ) {
                if let Some(current) = reply.try_child_value(0).and_then(|v| v.get::<String>()) {
                    if !previous.is_empty() && previous != current {
                        callback();
                    }
                    previous = current;
                }
            }
            glib::ControlFlow::Continue
        });
        self.watches.borrow_mut().push(source);
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        for source in self.watches.get_mut().drain(..) {
            source.remove();
        }
    }
}
