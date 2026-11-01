# Published feature revisions and merge preparation

Ken authorized committing and pushing the feature work. All implementation repositories are published on `feature/nyx-online-accounts`, including the separate public design-nexus/nyx-accounts repository. No main branch was changed and no pull request was created or merged.

Cross-repository dependencies now use immutable public Git revisions:

| Dependency | Revision |
|---|---|
| Accounts code | `ecbe31cbfe6688dd5b3d7803c58e433715427c57` |
| Core/native PIM | `1fa444e8c427f8d2f4b6fd6d1ace1c5298e4d623` |
| Cloud | `709168078a32a3fe93632574c00281fbe5cdd01f` |

Internal workspace paths remain internal. Targeted Core, Cloud and application consumer checks passed against the published dependencies. The coordinator's `docs/online-accounts-preview.toml` records the feature application revisions separately from supported `suite.toml`; normal release pins remain unchanged. Later Accounts documentation/installer commits do not change the pinned library code.

The installed FEATURE_COMMITS.json and FINAL_REPORT.md record feature branch heads. The preview installer never creates repositories, pushes, creates PRs or merges automatically. Feature publication does not authorize merging.

Before merging: configure and live-test both Google and personal Microsoft OAuth; validate representative Mail/Calendar/Contacts/Reminders/Files operations, credential lifecycle and real migration/data preservation; resolve material compatibility issues; run required focused checks; then obtain Ken's explicit merge authorization. Passing mocks, preview installation and implementation approval never authorize merging.
