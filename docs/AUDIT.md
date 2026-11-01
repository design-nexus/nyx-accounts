# Repository and dependency audit

The suite consists of independent repositories under `/home/ken/Projects`. All feature work is isolated on `feature/nyx-online-accounts`. No work is merged to main, no PR is created, and no provider credentials are committed.

| Repository | Existing architecture | Reuse / replacement |
|---|---|---|
| nyx-core | `nyx-accounts` was an in-process GIO GOA metadata/credential client; `nyx-eds` wrapped libedataserver/libecal; `nyx-sync` supplies provider-independent WebDAV | Preserve old crates for explicit migration. Add native `nyx-pim` storage using existing libical models, plus separate EDS export/import utility. |
| nyx-settings | Online Accounts, provider logos, GOA subprocess helper, Fastmail/Zoho presets, Cloud settings | Native account manager replaces GOA onboarding. Existing provider logos, compact widgets and Cloud controls are reused. GOA helper is excluded from the workspace. |
| nyx-mail | GLib engine, SQLite mail cache, queued operations/drafts, mature IMAP/SMTP/XOAUTH2, Microsoft Graph | Preserve engines and data schemas; replace discovery and connection credential source. Legacy ID mapping preserves mail-cache associations after an explicit metadata import. |
| nyx-calendar | EDS owns online/local events, recurrence expansion, invitation handling and reminders; local agenda projections | Native storage and application-owned provider adapters replace EDS runtime. Reuse libical parsing, rendering models, layout and control APIs. Full behavior parity is not yet achieved. |
| nyx-contacts | Existing SQLite local book and EDS online address books | Keep SQLite local book, display raw vCards from native store, and retain existing vCard UI conversion. |
| nyx-reminders | EDS task lists plus app-owned presentation and daemon | Native task storage, recurrence advancement, persisted dismiss/snooze state; existing UI/control APIs remain. |
| nyx-cloud | Rust service/public client; rclone over private RC sockets; Cloud1 metadata and private Notes sessions | Reuse existing file/Notes operations. Native-account linking and anonymous-file credentials avoid provider refresh tokens in rclone configs. |
| nyx-files | Cloud1 client and mounted storage UI | Existing protocol can consume new linked remotes without source changes. No Files GUI tests were run. |
| nyx-notes | Existing Cloud Notes sessions and separate WebDAV sync | Existing Cloud protocol remains. No source changes required for native linked remotes. |
| nyx | Independent-repo release/pin coordinator | Branch created, no implementation changes. Feature preview currently uses sibling path dependencies; publication/pinning is still required before release. |
| nyx-design | Shared Design 2.0 styles, provider-compatible theme tokens and app icons | Reused without modification. Existing untracked files preserved. |

## Authentication and security inventory

- GOA currently provides OAuth refresh, keyring access, service flags, account discovery and browser integration. It is not a native Nyx OAuth implementation.
- Mail already supported personal Graph mail operations; replacing its transport was unnecessary.
- Existing Calendar/Contacts Microsoft support depended on Evolution EWS/Microsoft365 backend modules. New preview app paths no longer require them.
- Cloud independently stored rclone OAuth credentials in its private config; native-linked remotes now use access-only credentials from Accounts. Existing external and legacy remotes remain untouched.
- Existing session D-Bus protocols, app control tools and MCP-facing command descriptions are preserved. No secret-returning MCP tool is added.
- Secret Service remains a desktop-standard dependency. A GNOME Keyring implementation can satisfy that interface without depending on GOA.

## Remaining references

Legacy GOA client/helper source, EDS FFI crates, historical app documentation and the explicit migration utility remain in the feature checkouts. Keeping them preserves migration capability; they must not be confused with new preview runtime dependencies. No installed system package is removed.
