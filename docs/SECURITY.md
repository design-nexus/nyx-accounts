# Credential boundary

The preview service owns refresh tokens, app passwords, and optional desktop client secrets in Secret Service using encrypted DH sessions. It never falls back to plaintext. SQLite contains validated nonsecret account metadata only.

D-Bus exposes metadata, configuration and single-use private Unix socket handles. Credential sessions expire after 30 seconds, have mode 0600, cap input size, validate peer UID/PID and the installed preview executable path, and enforce the executable's service grant and the account's enabled services. Responses contain only access tokens or protocol passwords, never refresh tokens or client secrets. Settings/CLI may write secrets but cannot request credential reads.

Management calls validate the D-Bus sender process against installed preview Settings/CLI/migration tool. Account metadata is readable within the user's bus session. This is a same-user desktop boundary, not a sandbox against a malicious process with access to the user's files, process memory or ptrace. Executable paths and a private bus reduce accidental exposure; they do not establish cryptographic application identity.

No credential values belong in logs, argv, environment variables, account JSON, issue reports or Git. Network error bodies are not included in OAuth UI errors. Credential structs and serialized private-session buffers zeroize on drop; third-party HTTP/JSON libraries may allocate transient copies.

All preview launches join one private bus, so refresh is serialized in a single daemon; rotated refresh tokens are saved before use. OAuth listeners bind loopback before opening the browser. State, PKCE, callback path, timeout and one-use completion protect authorization. Successful sign-in resolves the provider's verified identity rather than trusting an entered email address.

Google remote revocation is supported. Microsoft personal and app-password providers require manual provider-side revocation. Existing GOA credentials are never copied or invalidated.

Cloud keeps its rclone configuration in an inherited Linux memfd. Its loopback token endpoint accepts an unpredictable, process-lifetime capability for Files only; that value is not a provider refresh token. The accounts service still validates Cloud and the enabled Files grant on each renewal. Previously issued provider access tokens remain valid until expiry or provider revocation, so disabling a service cannot cryptographically invalidate an already-issued token.

OAuth profile binding uses verified provider subject IDs. A browser account switch cannot silently retarget an existing cache. Managers cannot inject refresh/access tokens through the secret-update API. The preview has a separate runtime directory as well as a separate bus; Secret Service alone uses the preserved desktop bus address, which is connection metadata rather than a credential. Per-service issuance controls do not cryptographically downscope every provider password/token, so trusted consumer code and the same-user boundary remain material limitations.

Application-owned PIM synchronization and conflict resolution share a database-specific OS lock across processes. Resolution verifies both provider and local snapshots inside a transaction; stale dialogs cannot silently discard newer edits. Both versions remain archived. The preview bus is started only by explicit launcher/command invocation and reused by later launches; it does not install an autostart user unit.

The private preview bus reads only the preview prefix’s Nyx D-Bus service files. It does not load desktop session activation directories, so calls cannot auto-start a second Secret Service, portal, or GVfs instance. Account credential access explicitly connects to the original desktop bus via `NYX_ACCOUNTS_SECRET_BUS_ADDRESS`; the original keyring and collections are never replaced or reset.

Explicit OAuth renewal is manager-only, serialized with credential and account changes, and returns only success. It refuses accounts requiring sign-in, so refreshing existing grants cannot clear a scope-change reauthorization requirement.

## Normal desktop installation

Normal managers/consumers are validated under ~/.local/bin and use the original desktop bus/Secret Service directly. Existing compatibility identifiers and credential attributes remain stable. Promotion retains account UUIDs without exporting secrets and guards subsequent preview launches. Normal and preview account engines must not run concurrently after promotion. Deferred/reauthorizing accounts retain metadata and caches but are excluded from background authentication attempts.
