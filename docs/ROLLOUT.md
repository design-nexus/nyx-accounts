# Google-first normal installation

Ken explicitly selected a Google-first rollout and waived the Microsoft live-test merge gate. Microsoft setup and live validation remain deferred, and the updated native applications cannot authenticate legacy Microsoft profiles. Keep GOA configuration, credentials and caches for recovery. This waiver does not itself authorize merging.

## Installation contracts

The normal service activates on the existing desktop session bus. Its installed managers and consumers live together under `~/.local/bin`, satisfying the existing executable/peer checks. `nyx install` installs Accounts before native consumers. A standalone service install is `bash install.sh accounts`; coordinated local builds use `--stage` followed by `--activate`.

The D-Bus name `io.github.design_nexus.nyx.Accounts.Preview`, data directories `nyx-accounts-preview` and `nyx-pim-preview`, and Secret Service application attribute `nyx-accounts-preview` remain stable compatibility identifiers. Normal and preview stores have different XDG roots. These identifiers are not user-facing names. Renaming them would invalidate tested clients and existing credential references without providing a user benefit.

No new keyring is created. Existing encrypted entries remain in the desktop Secret Service. Promotion copies metadata with the same native UUID; it never exports OAuth tokens or changes credential entries. After promotion the preview launcher refuses startup to prevent two copies of account metadata managing the same credentials.

## Coordinated migration

1. Record current Git revisions and installed binary hashes. Back up affected normal data/configuration, desktop/service files and binaries under a private timestamped rollout directory. Copy SQLite through its backup API. Preserve symlinks without traversing them.
2. Export legacy accounts, calendars, tasks and address books with `nyx-pim-migrate export FILE` on the desktop bus. Review warnings and retain originals, including EDS sources, pending changes and attachments.
3. Stop preview writers and the affected normal background engines for the handover. Do not close or reopen graphical apps automatically. Build/stage the coordinated binaries first.
4. Run `nyx-accounts-promote --legacy-export FILE` with normal data paths. It refuses existing native destinations, validates SQLite copies, retains native account UUIDs and binds matching legacy Mail IDs by provider plus email. The destination receipt records table counts. Ambiguous matches fail before destination installation.
5. Install the staged binaries using `install.sh --activate`. The installer updates regular application launchers/icons and registers desktop Accounts activation. Preserve normal Mail/Contacts stores; do not copy preview test-mail databases over them.
6. Import the legacy bundle with the installed migration tool. Existing native account matches use legacy ID or provider/email, avoiding a duplicate Google profile. Legacy calendars/address books become separate offline snapshots, preserving original UIDs/raw fields instead of re-uploading them.
7. Start only the necessary background engines for focused normal-session checks. Link Google Files through Cloud's existing native-account method; retain external rclone configuration and other remotes.
8. Compare native table counts, imported sources, local drafts/attachments and representative Google reads. Keep the preview tree and all originals for recovery. Do not revoke old grants or uninstall GOA/EDS packages.

## Recovery

Stop the new background engines before restoration. Restore saved binaries, desktop/service files and the previous affected configuration. Preserve new native stores separately; never overwrite newly composed mail or edited contacts with an old backup. The untouched original Mail/Contacts and EDS stores remain the legacy source of truth. Remove the preview PROMOTED marker only if intentionally returning to the isolated preview. Do not delete keyring entries or revoke Google to roll back.

## Merge

Prepare and test only on feature/nyx-online-accounts. Obtain explicit approval after presenting final revisions/results. Merge Accounts, Core, consumers and coordinator in that order, preserve immutable published pins, and never force-push. No pull request or public release is created automatically. Full Microsoft live validation remains follow-up work, not a claim of this release.

## Preserved storage mount convention

Existing and future Cloud accounts mount directly beneath the home directory, using the full display name. Native Google defaults to ~/Google Drive, Microsoft to ~/OneDrive and Nextcloud to ~/Nextcloud. Additional accounts use numbered suffixes. Existing Google Drive remote identity/preferences are retained during native conversion; original rclone configuration is not deleted or rewritten.
