# Development status and merge checklist

This is an isolated development preview. Real provider authentication and end-to-end application behavior remain unverified.

## Implemented

- Standalone account service, metadata store, Secret Service storage, service grants, caller validation and one-use private credential sessions.
- Google and personal Microsoft PKCE authorization, token exchange/refresh, cancellation, identity binding, health/errors, Google revocation and guided manual revocation for unsupported providers.
- Settings account/provider selection, guided setup, client/endpoint configuration, password updates, per-service toggles, multiple accounts, browser sign-in, status/error checks and removal.
- Mail uses native discovery and private credentials with its existing IMAP/SMTP and Graph engines; its data schemas remain unchanged.
- Native Calendar/Contacts/Reminders storage; raw payload and time-zone preservation; recurrence/exception handling, series splitting, recurring task advancement, persisted relative/absolute alarms and snoozes.
- Application-owned Google/Graph calendar/task read/write adapters, invitation responses, Google free/busy, contact reads, DAV discovery/capabilities and conditional calendar/task writes. Unsupported field conversions fail without discarding queued edits.
- Contacts retain the existing read-only online-book UI; incoming updates and remote-deletion tombstones preserve raw records.
- Cloud native links and anonymous rclone configuration with brokered access-token renewal. Provider refresh tokens never leave Accounts. Existing external/legacy remotes remain untouched.
- Explicit metadata/data export/import and local Mail/Contacts snapshot tools. These have not been run on user data.
- Preview installation with private bus/runtime and separate XDG stores, existing Settings UI, launcher/icon, and no automatic app opening or service restart.

## Incomplete or unverified

- Google and Microsoft credentials/accounts are not configured or live-tested. No production-verification claim is made.
- Live Secret Service locking/unlocking, Cloud token renewal/mounting and full provider CRUD require controlled account testing.
- Visual/accessibility, desktop notification and physical-input behavior have not been GUI-tested.
- Online contact editing remains read-only; unsupported API fields, uncommon recurrence/exception combinations and provider-specific task limitations are explicitly refused or retained for resolution.
- Personal Microsoft/CalDAV free-busy and CalDAV invitation-delivery behavior need provider-specific validation; unsupported operations return errors.
- Conflicts and ambiguous task creations are retained, but a complete graphical synchronization-conflict resolution flow is unfinished.
- Migration still requires validation of attachment references, unsent EDS operations, aliases, cache associations and representative user-approved data copies. Original data remains intact.
- Generic providers currently use one login identity/app password per account; provider-specific OAuth is implemented for Google/Microsoft, with app-password paths for the other supported providers.
- Development uses sibling path dependencies. Immutable public feature/release pins and suite release coordination are required before publication/merge.

## Security limits

Per-service grants constrain credential delivery to trusted Nyx consumers. Provider app passwords and OAuth grants may cover several enabled services; these grants are not a cryptographic sandbox between same-user applications. Previously issued access tokens can remain valid until expiry/provider revocation. See SECURITY.md for the desktop trust boundary.

## Merge gate

Do not merge, rebase or cherry-pick into main automatically. Both real Google and personal Microsoft authorization must succeed; Mail, Calendar, Contacts, tasks/storage and data preservation must be validated; required automated checks must pass; Ken must then explicitly authorize merging. Implementation approval and preview installation do not authorize a merge.
