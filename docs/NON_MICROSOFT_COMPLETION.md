# Non-Microsoft delivery and validation

## Scope completed

Native Nyx Accounts replaces GOA/EWS authentication in the isolated preview. The existing Settings Online Accounts layout, native application engines and Design 2.0 components are retained. All changes remain on feature/nyx-online-accounts. Microsoft remains deferred; no main merge or PR is authorized by this completion.

Google passed live authorization, refresh, Mail inbox/send/receive and attachment byte comparison; Calendar create/edit/delete, recurring series across DST, reminder preservation and free/busy; Contacts create/edit/delete; Tasks create/edit/complete/delete; and Drive browse/upload/direct-download/content comparison/delete. Disposable remote contacts, events, tasks and Drive files were removed. The two self-addressed test emails remain available for inspection.

Zoho, Fastmail, Nextcloud and generic-provider testing uses fixtures, as Ken requested. Provider-specific commercial account entitlements cannot be certified without those accounts. Protocol coverage includes verified-TLS DAV reads/writes/conflicts/discovery/scheduling; real disposable Dovecot IMAP/TLS and SMTP sink tests; and disposable local WebDAV/FUSE/cloud lifecycle fixtures. No mature protocol implementation was rewritten. JMAP remains deferred as originally planned.

## Completed fixes

- Account/PIM watches use the owning GLib context, including Calendar's worker.
- Google People compares CONTACT source revisions, retaining legacy raw-baseline compatibility.
- API DELETE requests have empty bodies.
- Provider downloads preserve a pending editor baseline.
- Recurring Google events retain canonical IANA zones even when libical supplies its built-in prefix, preserving local time across DST.
- The preview bus activates only its own Nyx services, preventing duplicate keyring, portal and GVfs activation. Existing desktop Secret Service is reused.
- Mail/Calendar/Reminders notification connections and actions use the original desktop bus; engine IPC remains private. All three live daemon connections and the existing notification server were checked.
- Health checks and explicit renewal cannot clear a pending OAuth sign-in requirement. Manager and consumer permission boundaries remain enforced.
- Release Mail ignores the fixture-account environment override; debug fixtures retain it.
- Credential tests cannot use the real desktop keyring by default or through --ignored alone; an independently supplied temporary fixture environment is required. No test provisions a keyring.

## Verification executed

- Accounts: 21 unit/controlled-server tests, plus application-runtime and private-IPC integration.
- Native PIM: 50 component tests plus the added canonical-zone recurrence regression.
- OAuth expiration: fresh tokens cause no request; expired and expiring tokens renew without user interaction; rotation and refresh-token retention are covered by a controlled server. The same exchange/keyring path also passed live Google renewal.
- OAuth revocation: controlled-server success/failure, correct POST transport, and error redaction. The user's real grant was not revoked.
- Native privacy/lifecycle: manager rejection, service grants, private sockets, restricted activation, shared concurrent bus startup/restart and service exit on bus loss.
- Mail: 9 disposable IMAP/TLS/SMTP network tests, 6 existing Graph fixtures, and 2 engine/session fixtures. The Graph fixture run does not constitute live Microsoft verification.
- Cloud: both disposable integration scripts passed, including password privacy, expired authentication, network reconnection, FUSE operations, unchanged external config, Notes revisions/stale writes and data-preserving removal. A GVariant boolean parsing bug in those scripts was fixed.
- Migration: disposable recurrence/exception/timezone/raw-field import, repeat-import edit protection, source preferences, unchanged source export and attachment reference/content.
- Essential release builds passed. Preview consumers' linked libraries showed no GOA, EWS or EDS runtime dependency. Legacy export/migration source remains intentionally available.

## Preservation and verification boundaries

Original application stores, existing external rclone configuration and provider records were not migrated, deleted or overwritten. The preview has separate data/config/runtime stores. Migration tools are explicit; fixture validation is complete, and applying them to production user data remains a separate deliberate action.

No manual pointer/keyboard/accessibility or notification-popup visual certification is claimed. The existing Settings layout was rendered and compared, and Ken confirmed its appearance. The other applications were exercised through installed engine/control interfaces. Natural wall-clock expiry was tested with a controlled clock/server rather than waiting an hour; live refresh exchange and subsequent provider access passed. Real keyring lock/unlock and revoking the user's Google grant were deliberately not exercised.

Known provider limits remain documented in PROVIDERS.md: no unsupported contact photo/favorite conversions, no generic universal app-password revocation API, no client-managed DAV invitation delivery, and safe refusal of unrepresentable recurrence/task fields. These do not represent silently discarded data.

Microsoft OAuth and live tests remain the remaining provider-development gate. The original explicit user merge approval and data-preservation gate remain in force even after Google success. No merge was performed.
