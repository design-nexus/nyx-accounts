# Published feature revisions and merge preparation

Ken authorized committing and pushing the feature work. All implementation repositories are published on `feature/nyx-online-accounts`, including the separate public design-nexus/nyx-accounts repository. No main branch was changed and no pull request was created or merged.

Cross-repository dependencies now use immutable public Git revisions:

| Dependency | Revision |
|---|---|
| Accounts code | See each consumer Cargo.toml and the feature inventory |
| Core/native PIM | See each consumer Cargo.toml and the feature inventory |
| Cloud | `709168078a32a3fe93632574c00281fbe5cdd01f` |

Internal workspace paths remain internal. Targeted Core, Cloud and application consumer checks passed against the published dependencies. The coordinator's `docs/online-accounts-preview.toml` records the feature application revisions separately from supported `suite.toml`; normal release pins remain unchanged. Later Accounts documentation/installer commits do not change the pinned library code.

The installed FEATURE_COMMITS.json and FINAL_REPORT.md record feature branch heads. The preview installer never creates repositories, pushes, creates PRs or merges automatically. Feature publication does not authorize merging.

Before merging: configure and live-test both Google and personal Microsoft OAuth; validate representative Mail/Calendar/Contacts/Reminders/Files operations, credential lifecycle and real migration/data preservation; resolve material compatibility issues; run required focused checks; then obtain Ken's explicit merge authorization. Passing mocks, preview installation and implementation approval never authorize merging.

Non-Microsoft delivery and validation are recorded in NON_MICROSOFT_COMPLETION.md. Feature heads and the installed build inventory are updated after publication; consumers retain immutable tested code pins, which can precede later diagnostic/documentation commits. Google is live-tested; other non-Microsoft providers use fixtures as requested. Microsoft and explicit merge approval remain required before main integration.

## Google-first rollout decision

Ken subsequently selected normal installation and explicitly waived the Microsoft live-test merge gate. Microsoft remains deferred and unverified. Final merge still requires explicit approval after the prepared installation/migration results are presented. See ROLLOUT.md.
