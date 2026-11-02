# Nyx Online Accounts

Native Rust account authentication service and shared client/protocol adapters, developed on `feature/nyx-online-accounts`.

Google has passed live authentication and application tests. Personal Microsoft authentication remains unverified and deferred for the Google-first rollout. Other providers use protocol fixtures. See the completion report for exact verification boundaries.

- [Repository/dependency audit](docs/AUDIT.md)
- [Implementation status and remaining work](docs/IMPLEMENTATION.md)
- [Provider setup instructions](docs/PROVIDERS.md)
- [Security boundaries](docs/SECURITY.md)
- [Publication and merge preparation](docs/RELEASE.md)

The service owns authentication, Secret Service credentials, refresh and grants. Application engines own synchronization. Refresh tokens never leave the service. D-Bus carries metadata and private-session handles only.

`install-preview.sh` builds and installs the feature binaries under `~/.local/opt/nyx-online-accounts-preview` and installs the Online Accounts Preview launcher. It does not start applications or enable systemd units. The launcher joins a shared private bus (starting it on explicit launch when needed) and uses separate XDG data/config/cache/state paths. Concurrent preview apps share account refresh and storage services.

`install.sh accounts` installs the standalone service on the normal desktop bus. `install.sh --stage` builds the coordinated normal application installation; `--activate` installs it after explicit data promotion. See [Google-first rollout and rollback](docs/ROLLOUT.md). Installation never opens graphical applications.

Explicit snapshot tooling is included: `nyx-pim-migrate export FILE` reads legacy EDS data and account metadata without copying secrets; `import FILE` creates native snapshots and account metadata requiring fresh authorization. `nyx-local-copy` copies local Mail/Contacts data only when a destination does not already exist. These operations are never run automatically against user data.

Credential-free checks:

```
cargo test --locked
cargo build --locked
dbus-run-session -- python3 tests/private_bus.py
python3 tests/preview.py
# After building the feature nyx-cloud debug binary:
python3 tests/service_lifecycle.py
# After building nyx-pim-migrate in nyx-core:
python3 tests/migration.py
# Optional unlocked desktop keyring check; uses disposable synthetic values:
cargo test --test keyring -- --ignored
```

All merges require successful real Google and personal Microsoft OAuth testing, validated integration/data preservation, passing required checks and explicit user approval. Implementation approval is not merge approval.
