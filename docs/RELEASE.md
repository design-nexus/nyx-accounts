# Feature publication and merge preparation

The installed preview uses sibling development paths. No main branch has been changed, no feature branch pushed, and no pull request created. Public immutable dependency pins cannot point at unpublished local commits. Replacing them prematurely would make standalone builds fail.

When feature publication is authorized, publish in dependency order while retaining `feature/nyx-online-accounts` everywhere:

1. Publish the separate `nyx-accounts` repository/feature commit.
2. Replace external Accounts path dependencies in the Core and Cloud feature checkouts with that immutable Git revision; build and publish those feature commits.
3. Replace external Accounts, native PIM and Cloud paths in the application feature checkouts with the published immutable revisions; retain internal workspace paths. Rebuild the preview and validate standalone consumers.
4. Update the suite coordinator's feature release manifest after these exact revisions exist remotely. Keep supported main release pins unchanged until merge approval.

Do not create a public repository, push branches, open a PR, or merge automatically as part of the preview installer. The installed FEATURE_COMMITS.json and FINAL_REPORT.md inventory the reviewable local commits. Publishing feature code is separate from approval to merge.

Before merging: configure and live-test both Google and personal Microsoft OAuth; validate representative Mail/Calendar/Contacts/Reminders/Files operations, credential lifecycle and migration/data preservation; resolve material compatibility issues; run the required focused checks; then obtain Ken's explicit merge authorization. Mock tests, preview installation and implementation approval never authorize merging.
