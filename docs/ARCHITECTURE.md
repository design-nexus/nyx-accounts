# Architecture and migration

## Ownership

`nyx-accountsd` owns account metadata, provider/client configuration, OAuth authorization, credential storage, refresh, revocation and service grants. It is a user service activated on the shared preview bus. Settings remains the sole graphical account manager.

The public Rust client uses synchronous GIO calls for metadata so application workers can use it inside GLib or Tokio without starting nested runtimes. `io.github.design_nexus.nyx.Accounts1` is the versioned metadata interface. Operations include registration/listing, service configuration, authorization/cancellation, health checks, revocation/removal and private-session creation.

Sensitive exchanges use one-use Unix sockets inside a private runtime directory. The daemon checks peer UID/PID, installed executable and its service grant, and rechecks account state before returning a credential. Managers can submit app passwords/client secrets, but cannot inject or retrieve OAuth tokens. Refresh tokens and client secrets never appear in metadata, signals, MCP tools or credential responses.

Secret Service stores encrypted credential payloads under the `nyx-accounts-preview` namespace. Explicit preview launches join one private bus, with startup serialized by an OS lock. Concurrent applications share the same Accounts refresh guard and Cloud service; a stopped bus is recreated on the next launch. Accounts exits on bus loss, cancelling its in-flight authorization jobs; Cloud shuts down its preview children and mounts instead of retaining an orphaned service. Lifetime file locks prevent overlapping service generations while cleanup finishes. The preview uses its own bus/runtime/data/config/cache/state paths while connecting to the existing desktop bus solely for Secret Service. No second keyring is silently provisioned. Missing/locked keyrings fail closed.

OAuth uses PKCE S256, random state, bounded loopback requests, one-use callbacks and cancellation/timeouts. Google and Microsoft use their own provider endpoints; Microsoft is restricted to `/consumers`. Verified provider subject IDs bind existing profiles, preventing another browser account from taking over an existing cache identity. Initial authorization must match the entered primary email. Refresh, mutation and deletion share a guard so revoked or deleted credentials are not recreated by a refresh race.

## Application data

Mail reuses its existing IMAP/SMTP/XOAUTH2 and Graph engines and existing cache/draft/outbox schemas. Native IDs can retain an explicit legacy ID association during migration.

`nyx-pim`, owned by nyx-core and consumed by Calendar/Contacts/Reminders, owns local storage and provider synchronization. The accounts daemon does not run it. SQLite preserves raw iCalendar, vCards, provider JSON and time-zone definitions alongside display/index data. Libical supplies calendar parsing and recurrence. CLDR 48 maps Windows time-zone identifiers to IANA identifiers; bundled Unicode licensing is retained.

Application-owned adapters use provider APIs or DAV. Edits queue locally. Conditional writes use known revisions; conflicts preserve both versions and stop an unsafe upload. API updates omit unchanged fields rather than rewriting unsupported provider properties. Task creates without provider idempotency support retain an uncertainty marker after ambiguous failures instead of blindly duplicating tasks. Settings can review both versions, keep local changes against the latest revision, accept the provider version, or preserve an offline copy. Uncertain creates require an explicit provider check before retrying. All decisions retain local and provider snapshots in resolution history; stale dialogs are rejected. A database-specific OS lock serializes synchronization and resolution across Calendar, Contacts, Reminders and Settings. Short SQLite write snapshots protect downloaded-resource merges and remote-deletion reconciliation from concurrent local edits; no network request runs inside these snapshots. Completed uploads clear only the exact payload/deletion snapshots they sent, retaining edits made during the request.

Cloud keeps rclone as the storage engine. Native links supply access-only configuration through an inherited Linux memfd. A process-lifetime capability lets rclone renew Files access through Cloud; Cloud obtains fresh access credentials from Accounts. This capability is not a provider refresh token. External and legacy rclone configurations are preserved. Existing Cloud1 and Notes session protocols remain compatible with Files/Notes.

## Contacts and recovery

Online contacts use the existing Contacts editor. A mature Rust vCard parser validates cards; property-level editor merges preserve raw unchanged fields, parameters, groups and folding. Native local edits remain marked as protected so repeat imports cannot overwrite them. App workers own all synchronization and obtain Contacts credentials through Accounts.

Google updates use explicit field masks and contact-source etags; Graph updates omit unchanged properties and discover nested contact folders. CardDAV uses the appropriate vCard content type, stable create paths, If-None-Match and revision-checked updates/deletes. Ambiguous API creations remain blocked until the user reviews the provider and chooses retry or offline recovery. API list omissions are confirmed through individual GETs before reconciliation because list results can lag writes. Google contact deletion includes a fresh revision preflight; atomic conditional delete behavior is not documented by that provider.

Settings reviews retained synchronization issues and prior versions. Recovery creates a separate offline book/calendar/task list and never writes to a provider. Account removal through Settings idempotently preserves visible offline PIM copies before deleting account metadata/credentials; original caches remain intact. The authentication daemon never performs this data work.

CalDAV scheduling follows RFC 6638: OPTIONS capability checks, successful PROPFIND properties, same-origin principal/outbox discovery, validated VFREEBUSY requests and per-recipient response checks. Unsupported or failed availability never becomes an empty “free” result. Personal Microsoft getSchedule is unsupported by Microsoft and is not emulated through enterprise endpoints.

## Explicit migration

1. `nyx-pim-migrate export FILE`, run on the existing desktop bus, reads legacy account metadata and exports complete event/task series, exceptions, time zones and address books through the existing APIs. It does not copy GOA tokens or GNOME client registrations.
2. Import the bundle on the preview bus. It creates separate native snapshots and account metadata requiring user-owned OAuth configuration or fresh app passwords. Existing preview edits are not overwritten by repeat imports.
3. `nyx-local-copy` can copy local Mail/Contacts data into an empty preview destination. SQLite uses its backup API; existing destinations and writable symlinks are refused.
4. Authorize new profiles and compare provider data, drafts, queued operations, recurrence, contacts and attachments with the preserved originals. Imported remote snapshots are not uploaded wholesale.
5. Keep original installations and stores until real provider testing and explicit merge authorization are complete.

Export cannot certify every EDS pending-upload state, and attachment references may still refer to original files. Original stores remain intact. No user data has been migrated automatically.
