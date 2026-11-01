# Google live validation — 2026-10-31

The configured Google account passed these checks through the installed, isolated preview applications and their native engines:

- Mail: IMAP inbox/folder download, SMTP message to the same account, and receipt of that uniquely marked message. The test email remains available for inspection.
- Calendar: provider calendars/events downloaded; disposable event creation and title edit were acknowledged by Google; deletion was acknowledged, and a subsequent complete refresh passed.
- Contacts: provider address book downloaded; disposable contact creation and note edit were acknowledged by Google; deletion passed. An earlier diagnostic contact was also cleaned up.
- Drive: native account linked and mounted; directory listing passed; a disposable file uploaded and appeared in a direct provider listing; a direct provider download matched the content; deletion was confirmed in the provider listing.

Final cache checks found no active or pending diagnostic contacts/events and no unresolved synchronization conflicts. The diagnostic Drive file is absent. No existing user records were intentionally changed. All account and application data stayed in preview paths. These were engine/API checks, not manual GUI interaction tests.

## Fixes found by live testing

1. Account and PIM watches now attach to their owning GLib context. Calendar's worker previously panicked while GTK owned the default context.
2. Google contact conflict detection uses the CONTACT source revision. Person representation tags can differ while the CONTACT source is unchanged. Existing cached baselines migrate through their retained raw source revision. Updates keep Google's documented metadata-source precondition rather than using that source tag as an HTTP representation tag. See [Google updateContact](https://developers.google.com/people/api/rest/v1/people/updateContact).
3. JSON API deletions send an empty request body instead of JSON `null`.
4. Downloading an unchanged provider version no longer overwrites the baseline of a pending local calendar/task edit. Provider-added organizer/link metadata previously made a simple title edit look like an unsupported metadata removal.

Focused context/revision/projection tests and the application release builds passed. Calendar and Contacts include explicit preview-only `--check-online` diagnostics; Contacts' diagnostic creates, edits and deletes a disposable record, so it is not a read-only check.

## Still unverified / merge gate

Live token expiry/refresh rotation, revocation, broader recurrence/attachment behavior, keyring lock/unlock interaction, real user migration, and manual application UI behavior still need validation. Microsoft personal remains unconfigured and untested while access to Entra is unavailable. Other providers remain fixture/source validated only. This is not a production certification.

All work remains on `feature/nyx-online-accounts`. Google success does not waive the Microsoft tests or the user's explicit merge approval. No main branch or supported suite release pins were changed.
