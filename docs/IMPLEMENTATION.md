# Development status and merge checklist

This is an isolated development preview. Non-Microsoft implementation and the requested Google/live plus other-provider fixture validation are complete. See NON_MICROSOFT_COMPLETION.md for evidence and verification boundaries. Microsoft remains deferred.

## Implemented

- Standalone account service, metadata store, Secret Service storage, service grants, caller validation and one-use private credential sessions.
- Google and personal Microsoft PKCE authorization, token exchange/refresh, cancellation, identity binding, health/errors, Google revocation and guided manual revocation for unsupported providers.
- Settings account/provider selection, guided setup, client/endpoint configuration, password updates, per-service toggles, multiple accounts, browser sign-in, status/error checks and removal.
- Mail uses native discovery and private credentials with its existing IMAP/SMTP and Graph engines; its data schemas remain unchanged.
- Native Calendar/Contacts/Reminders storage; raw payload and time-zone preservation; recurrence/exception handling, series splitting, recurring task advancement, persisted relative/absolute alarms and snoozes.
- Application-owned Google/Graph calendar/task read/write adapters, invitation responses, Google free/busy, contact reads, DAV discovery/capabilities and conditional calendar/task writes. Unsupported field conversions fail without discarding queued edits.
- Settings includes synchronization issue review, conditional local/provider resolution, offline copies, explicit uncertain-create retry and archived snapshots. Cross-process synchronization/resolution is serialized, and uploads retain concurrent local edits.
- Contacts reuse the existing editor for queued create/edit/delete in native books. Google People, personal Graph (including nested folders) and CardDAV adapters retain raw cards/provider JSON, enforce revision checks, protect stale editors, and retain conflicts or ambiguous creates. Unchanged grouped properties, parameters and folding remain intact.
- Settings provides archive review/recovery as separate offline copies. Removing an account through Settings first preserves visible offline copies of its cached events, tasks and contacts, including unsynced new items. Repeat imports preserve edited copies and source preferences.
- RFC 6638 CalDAV free/busy discovers the principal and scheduling outbox on the configured origin. Invitation responses require advertised automatic scheduling; unsupported client-managed delivery is refused. Provider HTTP 401 errors update account health.
- Cloud native links and anonymous rclone configuration with brokered access-token renewal. Provider refresh tokens never leave Accounts. Existing external/legacy remotes remain untouched.
- Explicit metadata/data export/import and local Mail/Contacts snapshot tools. These have not been run on user data.
- Preview installation with a shared private bus/runtime and separate XDG stores, existing Settings UI, launcher/icon, and no automatic app opening or service restart.

## Incomplete or unverified

- Google is configured and live-tested, including service CRUD, attachments, recurrence/DST and token renewal. Microsoft remains unconfigured/untested. Other providers use fixtures at Ken’s request; no production certification is claimed.
- Google service CRUD, Cloud mounting and renewal passed. Credential lifecycle/error boundaries are covered by fixtures. Real keyring lock/unlock and user-grant revocation were not performed; credential tests now require an explicit isolated fixture environment.
- Visual/accessibility, desktop notification and physical-input behavior have not been GUI-tested.
- API contact photo/favorite mutations and fields that cannot be represented safely are refused before queueing. DAV/local books retain those fields. Uncommon recurrence/exception combinations and provider-specific task limitations are explicitly refused or retained for resolution.
- Microsoft Graph getSchedule explicitly does not support personal accounts; no enterprise workaround is implemented. CalDAV scheduling requires a server advertising calendar-auto-schedule and appropriate privileges. Real provider delivery remains unverified.
- Resolution UI is implemented for retained event/task conflicts and ambiguous creations. Provider-specific fields or recurrence that cannot be represented still require manual correction or an offline copy; archived snapshots have a review/recovery browser.
- Synthetic migration tests cover recurrence, exceptions, custom time zones, raw fields, repeat imports, edited copies, source preferences and unchanged attachment references. Real attachment availability, unsent EDS operations, aliases and cache associations still require representative user-approved data testing. Original data remains intact.
- Generic providers currently use one login identity/app password per account; provider-specific OAuth is implemented for Google/Microsoft, with app-password paths for the other supported providers.
- Cross-repository Accounts, native PIM and Cloud dependencies now use published immutable Git feature revisions. Consumer checks passed against those revisions. The coordinator has a separate feature inventory; supported main release pins remain unchanged.

## Remaining provider constraints

Google contact deletion is preceded by a fresh individual read/revision comparison; its deleteContact API does not document atomic conditional deletion. The reviewed local version and raw provider baseline remain available locally, but concurrent provider edits during deletion require live compatibility validation. List omissions are confirmed by individual reads before cached API contacts are tombstoned, avoiding propagation-delay data loss.

## Security limits

Per-service grants constrain credential delivery to trusted Nyx consumers. Provider app passwords and OAuth grants may cover several enabled services; these grants are not a cryptographic sandbox between same-user applications. Previously issued access tokens can remain valid until expiry/provider revocation. See SECURITY.md for the desktop trust boundary.

## Merge gate

Do not merge, rebase or cherry-pick into main automatically. Both real Google and personal Microsoft authorization must succeed; Mail, Calendar, Contacts, tasks/storage and data preservation must be validated; required automated checks must pass; Ken must then explicitly authorize merging. Implementation approval and preview installation do not authorize a merge.

## Google-first rollout decision

Ken subsequently selected normal installation and explicitly waived the Microsoft live-test merge gate. Microsoft remains deferred and unverified. Final merge still requires explicit approval after the prepared installation/migration results are presented. See ROLLOUT.md.
