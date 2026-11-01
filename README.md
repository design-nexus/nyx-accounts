# Nyx Online Accounts

Native Rust account authentication service and shared client/protocol adapters, developed on `feature/nyx-online-accounts`.

This is an isolated development preview. It is not merge-ready and Google/Microsoft have not been live-verified. Existing applications, credentials and data remain in place.

- [Repository/dependency audit](docs/AUDIT.md)
- [Implementation status and remaining work](docs/IMPLEMENTATION.md)
- [Provider setup instructions](docs/PROVIDERS.md)
- [Security boundaries](docs/SECURITY.md)

The service owns authentication, Secret Service credentials, refresh and grants. Application engines own synchronization. Refresh tokens never leave the service. D-Bus carries metadata and private-session handles only.

`install-preview.sh` builds and installs the feature binaries under `~/.local/opt/nyx-online-accounts-preview` and installs the Online Accounts Preview launcher. It does not start applications or enable systemd units. The launcher creates a private bus and separate XDG data/config/cache/state paths.

Explicit snapshot tooling is included: `nyx-pim-migrate export FILE` reads legacy EDS data and account metadata without copying secrets; `import FILE` creates native snapshots and account metadata requiring fresh authorization. `nyx-local-copy` copies local Mail/Contacts data only when a destination does not already exist. These operations are never run automatically against user data.

Credential-free checks:

```
cargo test --locked
cargo build --locked
dbus-run-session -- python3 tests/private_bus.py
```

All merges require successful real Google and personal Microsoft OAuth testing, validated integration/data preservation, passing required checks and explicit user approval. Implementation approval is not merge approval.
